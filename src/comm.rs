use std::io::{Cursor, Read, Write};

use binrw::prelude::*;

use crate::error::Error;

/// The current version of the protocol
pub const VERSION: u8 = 0x00;

/// Reply command code
pub const COMMAND_REPLY: u16 = 0x0000;
/// Error command code
pub const COMMAND_ERROR: u16 = 0x0001;

/// Ping command code
pub const COMMAND_PING: u16 = 0x0002;

/// Read shared context command code
pub const COMMAND_READ_SHARED_CONTEXT: u16 = 0x0100;
/// Write shared context command code
pub const COMMAND_WRITE_SHARED_CONTEXT: u16 = 0x0101;
/// Write shared context unconfirmed command code
pub const COMMAND_WRITE_SHARED_CONTEXT_UNCONFIRMED: u16 = 0x0102;

/// Standard commands
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Command {
    /// Reply, may carry data
    Reply,
    /// Error, carries error code (u16) and optional UTF-8 message
    Error,
    /// Ping, carries no data
    Ping,
    /// Read shared context, carries [`RawDataHeader`] and the data
    ReadSharedContext,
    /// Write shared context, carries [`RawDataHeader`] and the data
    WriteSharedContext,
    /// Write shared context with no reply (push), carries [`RawDataHeader`] and the data
    WriteSharedContextUnconfirmed,

    /// Custom commands starting from 0x8000
    Other(u16),
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
    /// Get the command code
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

/// Packet structure
#[derive(Debug, Clone)]
pub struct Packet {
    frame: Frame,
    data_len: usize,
}

impl Packet {
    /// Create a new packet
    pub fn new(frame: Frame, data_len: usize) -> Self {
        Self { frame, data_len }
    }
    /// Write the packet to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        let packet_header = PacketHeader::new(u32::try_from(self.data_len + Frame::SIZE)?);
        let mut buffer = [0u8; PacketHeader::SIZE + Frame::SIZE];
        let mut cursor = Cursor::new(&mut buffer[..]);
        packet_header.write(&mut cursor)?;
        self.frame.write_le(&mut cursor)?;
        writer.write_all(&buffer)?;
        Ok(())
    }
    /// Read a packet from a reader
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
    /// The packet frame data
    pub fn frame(&self) -> &Frame {
        &self.frame
    }
    /// The packet data length
    pub fn data_len(&self) -> usize {
        self.data_len
    }
    /// The full packet size (header + frame + data)
    pub fn size_full(&self) -> usize {
        PacketHeader::SIZE + Frame::SIZE + self.data_len
    }
}

/// Packet header structure
#[binrw]
#[brw(little, magic = b"RD")]
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    /// The protocol version
    pub version: u8,
    /// The size of the packet including the frame and data
    pub size: u32,
}

impl PacketHeader {
    /// The size of the packet header
    pub const SIZE: usize = 7;

    /// Create a new packet header
    pub fn new(size: u32) -> Self {
        Self {
            version: VERSION,
            size,
        }
    }

    /// Check the protocol version is supported
    pub fn check_version(&self) -> Result<(), Error> {
        if self.version != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        Ok(())
    }
}

/// Frame structure
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct Frame {
    /// The source host address (id)
    pub source: u32,
    /// The target host address (id)
    pub target: u32,
    /// The frame id
    pub id: u32,
    /// The id of the frame this is a reply to, 0 if not a reply
    pub in_reply_to: u32,
    /// The command code
    pub command: Command, // u16
}

impl Frame {
    /// The size of the frame header
    pub const SIZE: usize = 19;
    /// Convert the frame to a reply frame
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

/// Raw data header structure
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct RawDataHeader {
    /// The register address
    pub register: u32,
    /// The offset within the register
    pub offset: u32,
    /// The size of the data
    pub size: u32,
}

impl RawDataHeader {
    /// The size of the raw data header
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
