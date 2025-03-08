use crate::comm::{Command, Frame, Packet, RawDataHeader};
use crate::context::RpdoContext;
use crate::error::Error;
use crate::host::SyncHost;
use crate::Result;
use binrw::prelude::*;
use std::io::{Cursor, Read, Write};
use std::mem;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

const MAX_UDP_PACKET_SIZE: usize = 16384;

pub struct UdpStream {
    socket: UdpSocket,
    peer: Option<SocketAddr>,
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    mtu: usize,
}

impl UdpStream {
    pub fn create(bind: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        Ok(Self {
            socket,
            peer: None,
            read_buffer: Vec::new(),
            write_buffer: Vec::new(),
            mtu: MAX_UDP_PACKET_SIZE,
        })
    }
    pub fn try_with_mtu(mut self, max_packet_size: usize) -> Result<Self> {
        if max_packet_size > MAX_UDP_PACKET_SIZE {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "MTU too large",
            )));
        }
        self.mtu = max_packet_size;
        Ok(self)
    }
    pub fn set_peer(&mut self, peer: impl ToSocketAddrs) -> Result<()> {
        let peer = peer
            .to_socket_addrs()?
            .next()
            .ok_or(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid peer address",
            )))?;
        self.peer = Some(peer);
        Ok(())
    }
}

impl Read for UdpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.read_buffer.is_empty() {
            // must be read in a single packet
            let mut buf = [0; MAX_UDP_PACKET_SIZE];
            let (size, addr) = self.socket.recv_from(&mut buf)?;
            self.read_buffer.extend_from_slice(&buf[..size]);
            self.peer = Some(addr);
        }
        let size = std::cmp::min(buf.len(), self.read_buffer.len());
        buf[..size].copy_from_slice(&self.read_buffer[..size]);
        self.read_buffer.drain(..size);
        Ok(size)
    }
}

impl Write for UdpStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let data = mem::take(&mut self.write_buffer);
        let Some(peer) = self.peer else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No peer address",
            ));
        };
        if data.len() > self.mtu {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Data too large",
            ));
        }
        self.socket.send_to(&data, peer)?;
        Ok(())
    }
}

pub struct SimpleClient<S>
where
    S: Read + Write,
{
    request_id: u32,
    stream: S,
    target_id: u32,
    data_buf: Vec<u8>,
}

impl<S> SimpleClient<S>
where
    S: Read + Write,
{
    pub fn new(stream: S, target_id: u32) -> Self {
        Self {
            request_id: 0,
            stream,
            target_id,
            data_buf: Vec::new(),
        }
    }
    pub fn ping(&mut self) -> Result<()> {
        self.communicate(Command::Ping, &[], true)?;
        Ok(())
    }
    pub fn read_register(&mut self, register: u32, offset: u32, size: u32) -> Result<Vec<u8>> {
        let raw_data_header = RawDataHeader {
            register,
            offset,
            size,
        };
        let mut buf = Cursor::new(Vec::new());
        raw_data_header.write(&mut buf)?;
        let Some(v) = self.communicate(Command::ReadSharedContext, buf.get_ref(), true)? else {
            return Err(Error::InvalidReply);
        };
        Ok(v)
    }
    pub fn write_register(&mut self, register: u32, offset: u32, data: &[u8]) -> Result<()> {
        let raw_data_header = RawDataHeader {
            register,
            offset,
            size: u32::try_from(data.len())?,
        };
        let mut buf = Cursor::new(Vec::new());
        raw_data_header.write(&mut buf)?;
        buf.write_all(data)?;
        self.communicate(Command::WriteSharedContext, buf.get_ref(), true)?;
        Ok(())
    }
    pub fn communicate(
        &mut self,
        command: Command,
        data: &[u8],
        wait_reply: bool,
    ) -> Result<Option<Vec<u8>>> {
        let request_id = self.request_id;
        self.request_id += 1;
        let frame = Frame {
            source: 0,
            target: self.target_id,
            id: request_id,
            in_reply_to: 0,
            command,
        };
        let packet = Packet::new(frame, data.len());
        packet.write_to(&mut self.stream)?;
        self.stream.write_all(data)?;
        self.stream.flush()?;
        if !wait_reply {
            return Ok(None);
        }
        let packet = Packet::read_from(&mut self.stream)?;
        let data_len = packet.data_len();
        self.data_buf.resize(data_len, 0);
        self.stream.read_exact(&mut self.data_buf)?;
        let frame = packet.frame();
        if frame.target != 0 || frame.in_reply_to != request_id {
            return Err(Error::InvalidReply);
        }
        Ok(Some(self.data_buf.clone()))
    }
}

pub struct SimpleServerProcessor<CTX, HOST, S>
where
    CTX: RpdoContext,
    HOST: SyncHost<Context = CTX>,
    S: Read + Write,
{
    host: HOST,
    stream: S,
    data_buf: Vec<u8>,
}

impl<CTX, HOST, S> SimpleServerProcessor<CTX, HOST, S>
where
    CTX: RpdoContext,
    HOST: SyncHost<Context = CTX>,
    S: Read + Write,
{
    pub fn new(host: HOST, stream: S) -> Self
    where
        HOST: SyncHost,
    {
        Self {
            host,
            stream,
            data_buf: Vec::new(),
        }
    }

    pub fn process_next(&mut self) -> Result<()> {
        let packet = Packet::read_from(&mut self.stream)?;
        self.data_buf.resize(packet.data_len(), 0);
        self.stream.read_exact(&mut self.data_buf)?;
        let frame = packet.frame();
        if let Some((reply, data)) = self.host.process_frame(frame, &self.data_buf)? {
            let packet = Packet::new(reply, data.len());
            packet.write_to(&mut self.stream)?;
            self.stream.write_all(&data)?;
            self.stream.flush()?;
        }
        Ok(())
    }
}
