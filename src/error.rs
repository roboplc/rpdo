use core::fmt;

/// Error code for unknown host
pub const ERR_UNKNOWN_HOST: u16 = 0x0001;
/// Error code for invalid command
pub const ERR_INVALID_COMMAND: u16 = 0x0002;
/// Error code for invalid register
pub const ERR_INVALID_REGISTER: u16 = 0x0003;
/// Error code for invalid offset
pub const ERR_INVALID_OFFSET: u16 = 0x0004;
/// Error code for invalid reply
pub const ERR_INVALID_REPLY: u16 = 0x0005;
/// Error code for overflow
pub const ERR_OVERFLOW: u16 = 0x0006;
/// Error code for invalid version
pub const ERR_INVALID_VERSION: u16 = 0x0007;
/// Error code for I/O error
pub const ERR_IO: u16 = 0x0008;
/// Error code for invalid data
pub const ERR_INVALID_DATA: u16 = 0x0009;
/// Error code for failed data packing/unpacking
pub const ERR_PACKER: u16 = 0x0010;
/// Error code for all other errors
pub const ERR_FAILED: u16 = 0x0000;

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Host unknown
    #[error("Host unknown")]
    UnknownHost,
    /// Invalid command
    #[error("Invalid command")]
    InvalidCommand,
    /// Invalid register
    #[error("Invalid register")]
    InvalidRegister,
    /// Invalid register offset
    #[error("Invalid offset")]
    InvalidOffset,
    /// Invalid reply received
    #[error("Invalid reply")]
    InvalidReply,

    /// Overflow (e.g. register size)
    #[error("Overflow")]
    Overflow,
    /// Unsupported protocol version
    #[error("Invalid version")]
    UnsupportedVersion,
    /// I/O error
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    /// Invalid data
    #[error("Invalid data")]
    InvalidData,
    /// Packer/Unpacker error
    #[error("Packer: {0}")]
    Packer(#[from] binrw::Error),
    /// Failed
    #[error("Failed: {0}")]
    Failed(String),
}

impl From<Error> for Vec<u8> {
    fn from(err: Error) -> Self {
        let mut buf = Vec::<u8>::with_capacity(2);
        buf.extend_from_slice(&err.code().to_le_bytes());
        match err {
            Error::Io(e) => buf.extend_from_slice(e.to_string().as_bytes()),
            Error::Packer(e) => buf.extend_from_slice(e.to_string().as_bytes()),
            Error::Failed(msg) => buf.extend_from_slice(msg.as_bytes()),
            _ => (),
        }
        buf
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(_: std::num::TryFromIntError) -> Self {
        Self::Overflow
    }
}

impl From<&[u8]> for Error {
    fn from(slice: &[u8]) -> Self {
        if slice.len() < 2 {
            return Error::Failed(String::new());
        }
        let code = u16::from_le_bytes(slice[..2].try_into().unwrap());
        let msg = std::str::from_utf8(&slice[2..]).unwrap_or_default();
        match code {
            ERR_UNKNOWN_HOST => Self::UnknownHost,
            ERR_INVALID_COMMAND => Self::InvalidCommand,
            ERR_INVALID_REGISTER => Self::InvalidRegister,
            ERR_INVALID_OFFSET => Self::InvalidOffset,
            ERR_INVALID_REPLY => Self::InvalidReply,
            ERR_OVERFLOW => Self::Overflow,
            ERR_INVALID_VERSION => Self::UnsupportedVersion,
            ERR_IO => Self::Io(std::io::Error::new(std::io::ErrorKind::Other, msg)),
            ERR_INVALID_DATA => Self::InvalidData,
            ERR_FAILED => Self::Failed(msg.to_string()),
            _ => Self::Failed(format!("Unknown error code: 0x{:04X}", code)),
        }
    }
}

impl From<u16> for Error {
    fn from(e: u16) -> Self {
        match e {
            ERR_UNKNOWN_HOST => Self::UnknownHost,
            ERR_INVALID_COMMAND => Self::InvalidCommand,
            ERR_INVALID_REGISTER => Self::InvalidRegister,
            ERR_INVALID_OFFSET => Self::InvalidOffset,
            ERR_INVALID_REPLY => Self::InvalidReply,
            ERR_OVERFLOW => Self::Overflow,
            ERR_INVALID_VERSION => Self::UnsupportedVersion,
            ERR_IO => Self::Io(std::io::Error::new(std::io::ErrorKind::Other, "I/O error")),
            ERR_INVALID_DATA => Self::InvalidData,
            _ => Self::Failed(format!("Unknown error code: 0x{:04X}", e)),
        }
    }
}

impl Error {
    /// Get the error code
    pub const fn code(&self) -> u16 {
        match self {
            Self::UnknownHost => ERR_UNKNOWN_HOST,
            Self::InvalidCommand => ERR_INVALID_COMMAND,
            Self::InvalidRegister => ERR_INVALID_REGISTER,
            Self::InvalidOffset => ERR_INVALID_OFFSET,
            Self::InvalidReply => ERR_INVALID_REPLY,
            Self::Overflow => ERR_OVERFLOW,
            Self::UnsupportedVersion => ERR_INVALID_VERSION,
            Self::Io(_) => ERR_IO,
            Self::InvalidData => ERR_INVALID_DATA,
            Self::Packer(_) => ERR_PACKER,
            Self::Failed(_) => ERR_FAILED,
        }
    }
    /// Create a failed error
    pub fn failed<D: fmt::Display>(msg: D) -> Self {
        Self::Failed(msg.to_string())
    }
}
