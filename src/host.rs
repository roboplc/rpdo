use binrw::prelude::*;
use std::io::Cursor;
use std::sync::{atomic, Arc};

use crate::comm::{Command, Frame, RawDataHeader};
use crate::context::RpdoContext;
use crate::error::Error;
use crate::Result;

pub trait CustomCommandHandler: Send + Sync + 'static {
    fn handle(&self, frame: &Frame, data: &[u8]) -> Result<Option<Vec<u8>>>;
}

#[allow(clippy::module_name_repetitions)]
pub trait SyncHost {
    type Context: RpdoContext;

    fn host_id_matches(&self, frame: &Frame) -> bool;
    fn create_frame(&self, target: u32, in_reply_to: u32, command: Command) -> Frame;
    fn process_frame(&self, frame: &Frame, data: &[u8]) -> Result<Option<(Frame, Vec<u8>)>>;
}

#[derive(Clone)]
pub struct Host<CTX>
where
    CTX: RpdoContext,
{
    id: u32,
    inner: Arc<HostInner<CTX>>,
    custom_command_handler: Option<Arc<dyn CustomCommandHandler>>,
}

impl<CTX> Host<CTX>
where
    CTX: RpdoContext,
{
    pub fn new(id: u32, context: CTX) -> Self {
        Self {
            id,
            inner: Arc::new(HostInner {
                next_frame_id: atomic::AtomicU32::new(0),
                context,
            }),
            custom_command_handler: None,
        }
    }
    pub fn with_custom_command_handler(
        mut self,
        custom_command_handler: Arc<dyn CustomCommandHandler>,
    ) -> Self {
        self.custom_command_handler = Some(custom_command_handler);
        self
    }
}

impl<CTX> SyncHost for Host<CTX>
where
    CTX: RpdoContext,
{
    type Context = CTX;
    fn create_frame(&self, target: u32, in_reply_to: u32, command: Command) -> Frame {
        let frame_id = self
            .inner
            .next_frame_id
            .fetch_add(1, atomic::Ordering::Relaxed);
        Frame {
            source: self.id,
            target,
            id: frame_id,
            in_reply_to,
            command,
        }
    }

    fn host_id_matches(&self, frame: &Frame) -> bool {
        frame.target == self.id || frame.target == 0
    }

    fn process_frame(&self, frame: &Frame, data: &[u8]) -> Result<Option<(Frame, Vec<u8>)>> {
        match frame.command {
            Command::Reply => {
                return Ok(None);
            }
            Command::Error => {
                let err: Error = Error::from(data);
                eprintln!("host: {} error: {:?}", self.id, err);
                return Ok(None);
            }
            _ => {}
        }
        if !self.host_id_matches(frame) {
            return Ok(Some((
                self.create_frame(frame.source, frame.id, Command::Error),
                Error::UnknownHost.into(),
            )));
        }
        match frame.command {
            Command::Ping => Ok(Some((
                self.create_frame(frame.source, frame.id, Command::Reply),
                vec![],
            ))),
            Command::ReadSharedContext => {
                let mut cursor = Cursor::new(data);
                let raw_data_header = RawDataHeader::read(&mut cursor)?;
                match self.inner.context.get_bytes(
                    raw_data_header.register,
                    raw_data_header.offset,
                    raw_data_header.size,
                ) {
                    Ok(v) => Ok(Some((
                        self.create_frame(frame.source, frame.id, Command::Reply),
                        v,
                    ))),
                    Err(e) => Ok(Some((
                        self.create_frame(frame.source, frame.id, Command::Error),
                        e.into(),
                    ))),
                }
            }
            Command::WriteSharedContext | Command::WriteSharedContextUnconfirmed => {
                let mut cursor = Cursor::new(data);
                let raw_data_header = RawDataHeader::read(&mut cursor)?;
                let raw_data = &data[RawDataHeader::SIZE..];
                if raw_data_header.size != u32::try_from(raw_data.len())? {
                    return Err(Error::InvalidData);
                }
                match self.inner.context.set_bytes(
                    raw_data_header.register,
                    raw_data_header.offset,
                    raw_data,
                ) {
                    Ok(()) => {
                        if frame.command == Command::WriteSharedContext {
                            Ok(Some((
                                self.create_frame(frame.source, frame.id, Command::Reply),
                                vec![],
                            )))
                        } else {
                            Ok(None)
                        }
                    }
                    Err(e) => Ok(Some((
                        self.create_frame(frame.source, frame.id, Command::Error),
                        e.into(),
                    ))),
                }
            }
            _ => {
                if let Some(ref custom_command_handler) = self.custom_command_handler {
                    match custom_command_handler.handle(frame, data) {
                        Ok(Some(v)) => Ok(Some((
                            self.create_frame(frame.source, frame.id, Command::Reply),
                            v,
                        ))),
                        Ok(None) => Ok(None),
                        Err(e) => Ok(Some((
                            self.create_frame(frame.source, frame.id, Command::Error),
                            e.into(),
                        ))),
                    }
                } else {
                    Ok(Some((
                        self.create_frame(frame.source, frame.id, Command::Error),
                        Error::InvalidCommand.into(),
                    )))
                }
            }
        }
    }
}

struct HostInner<CTX>
where
    CTX: RpdoContext,
{
    next_frame_id: atomic::AtomicU32,
    context: CTX,
}
