use std::io::{Cursor, Read, Write};

use binrw::prelude::*;

use crate::error::Error;

pub const VERSION: u8 = 0x00;

pub const OK: u8 = 0x00;

pub const COMMAND_REPLY: u16 = 0x0000;
pub const COMMAND_ERROR: u16 = 0x0001;

pub const COMMAND_PING: u16 = 0x0002;

pub const COMMAND_READ_SHARED_CONTEXT: u16 = 0x0100;
pub const COMMAND_WRITE_SHARED_CONTEXT: u16 = 0x0101;
pub const COMMAND_WRITE_SHARED_CONTEXT_UNCONFIRMED: u16 = 0x0102;

// Standard frames
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Command {
    Reply, // may carry data
    Error, // carries error code (u16) and optional UTF-8 message

    Ping, // carries no data

    ReadSharedContext,             // carries RawData
    WriteSharedContext,            // carries RawData
    WriteSharedContextUnconfirmed, // carries RawData, same as WriteSharedContext but with no reply

    Other(u16), // custom commands starting from 0x8000
}

impl From<u16> for Command {
    fn from(value: u16) -> Self {
        match value {
            COMMAND_REPLY => Self::Reply,
            COMMAND_PING => Self::Ping,
            COMMAND_ERROR => Self::Error,
            COMMAND_READ_SHARED_CONTEXT => Self::ReadSharedContext,
            COMMAND_WRITE_SHARED_CONTEXT => Self::WriteSharedContext,
            COMMAND_WRITE_SHARED_CONTEXT_UNCONFIRMED => Self::WriteSharedContextUnconfirmed,
            _ => Self::Other(value),
        }
    }
}

impl Command {
    pub fn code(self) -> u16 {
        match self {
            Self::Reply => COMMAND_REPLY,
            Self::Ping => COMMAND_PING,
            Self::Error => COMMAND_ERROR,
            Self::ReadSharedContext => COMMAND_READ_SHARED_CONTEXT,
            Self::WriteSharedContext => COMMAND_WRITE_SHARED_CONTEXT,
            Self::WriteSharedContextUnconfirmed => COMMAND_WRITE_SHARED_CONTEXT_UNCONFIRMED,
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Packet {
    frame: Frame,
    data_len: usize,
}

impl Packet {
    pub fn new(frame: Frame, data_len: usize) -> Self {
        Self { frame, data_len }
    }
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let packet_header = PacketHeader::new(u32::try_from(self.data_len + Frame::SIZE)?);
        let mut buffer = [0u8; PacketHeader::SIZE + Frame::SIZE];
        let mut cursor = Cursor::new(&mut buffer[..]);
        packet_header.write(&mut cursor)?;
        self.frame.write_le(&mut cursor)?;
        writer.write_all(&buffer)?;
        Ok(())
    }
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut header_buffer = [0u8; PacketHeader::SIZE];
        reader.read_exact(&mut header_buffer)?;
        let header = PacketHeader::read(&mut Cursor::new(&header_buffer))?;
        if header.version != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if header.size < u32::try_from(Frame::SIZE)? {
            return Err(Error::InvalidData);
        }
        let mut frame_buffer = vec![0u8; Frame::SIZE];
        reader.read_exact(&mut frame_buffer)?;
        let frame = Frame::read(&mut Cursor::new(&frame_buffer))?;
        Ok(Self {
            frame,
            data_len: usize::try_from(header.size)? - Frame::SIZE,
        })
    }
    pub fn frame(&self) -> &Frame {
        &self.frame
    }
    pub fn data_len(&self) -> usize {
        self.data_len
    }
}

#[binrw]
#[brw(little, magic = b"RD")]
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub version: u8,
    pub size: u32,
}

impl PacketHeader {
    pub const SIZE: usize = 7;

    pub fn new(size: u32) -> Self {
        Self {
            version: VERSION,
            size,
        }
    }

    pub fn check_version(&self) -> Result<(), Error> {
        if self.version != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        Ok(())
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct Frame {
    pub source: u32,
    pub target: u32,
    pub id: u32,
    pub in_reply_to: u32,
    pub command: Command, // u16
}

impl Frame {
    pub const SIZE: usize = 19;
    pub fn to_reply(&self, id: u32, error: bool) -> Self {
        Self {
            source: self.target,
            target: self.source,
            id,
            in_reply_to: self.id,
            command: if error {
                Command::Error
            } else {
                Command::Reply
            },
        }
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct RawDataHeader {
    pub register: u32,
    pub offset: u32,
    pub size: u32,
}

impl RawDataHeader {
    pub const SIZE: usize = 12;
}

// Additinal impls for Command

impl BinRead for Command {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        u16::read_options(reader, endian, args).map(Self::from)
    }
}

impl BinWrite for Command {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        self.code().write_options(writer, endian, args)
    }
}
