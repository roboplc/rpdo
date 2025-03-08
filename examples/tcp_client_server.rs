use binrw::prelude::*;
use rpdo::comm::Command;
use std::{
    net::TcpListener,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

// A custom command handler, optional. Custom commands start from 0x8000.
struct CommandHandler {}

const COMMAND_POKE: u16 = 0x8000;
const COMMAND_REVERSE: u16 = 0x8001;

#[repr(u16)]
enum CustomCommand {
    Poke = COMMAND_POKE,
    Reverse = COMMAND_REVERSE,
}

impl TryFrom<Command> for CustomCommand {
    type Error = rpdo::Error;
    fn try_from(command: Command) -> Result<Self, Self::Error> {
        match command {
            Command::Other(COMMAND_POKE) => Ok(Self::Poke),
            Command::Other(COMMAND_REVERSE) => Ok(Self::Reverse),
            _ => Err(Self::Error::InvalidCommand),
        }
    }
}

impl From<CustomCommand> for Command {
    fn from(command: CustomCommand) -> Self {
        Command::Other(command as u16)
    }
}

impl rpdo::host::CustomCommandHandler for CommandHandler {
    fn handle(&self, frame: &rpdo::comm::Frame, data: &[u8]) -> rpdo::Result<Option<Vec<u8>>> {
        let custom_command = CustomCommand::try_from(frame.command)?;
        match custom_command {
            CustomCommand::Poke => {
                let s = std::str::from_utf8(data).map_err(rpdo::Error::failed)?;
                println!("Poked: {}", s);
                Ok(None)
            }
            CustomCommand::Reverse => {
                let mut buf = Vec::with_capacity(data.len());
                buf.extend_from_slice(data);
                buf.reverse();
                Ok(Some(buf))
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = rpdo::context::Basic::new(1000, 0, true);
    let host = rpdo::host::Host::new(1, context.clone())
        .with_custom_command_handler(Arc::new(CommandHandler {}));
    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:3003").unwrap();
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            stream.set_nodelay(true).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut processor = rpdo::io::SimpleServerProcessor::new(host.clone(), stream);
            thread::spawn(move || loop {
                if let Err(e) = processor.process_next() {
                    eprintln!("error: {:?}", e);
                    break;
                }
            });
        }
    });
    thread::sleep(Duration::from_secs(1));
    let stream = std::net::TcpStream::connect("127.0.0.1:3003")?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut client = rpdo::io::SimpleClient::new(stream, 0);
    let mut counter: u32 = 0;
    loop {
        counter += 1;
        // standard commands
        client.ping()?;
        println!("ping");
        let now = Instant::now();
        client.write_register(0, 0, &counter.to_le_bytes())?;
        println!("write elapsed: {:?}", now.elapsed());
        let now = Instant::now();
        let read_back = u32::from_le_bytes(client.read_register(0, 0, 4)?.try_into().unwrap());
        let read_back_check: u32 = context.get(0, 0, 4).unwrap();
        assert_eq!(read_back, read_back_check);
        println!(
            "{}/{}, read elapsed: {:?}",
            counter,
            read_back,
            now.elapsed()
        );
        println!("----------------");
        // custom commands
        client.communicate(CustomCommand::Poke.into(), b"Hello", false)?;
        let response = client
            .communicate(CustomCommand::Reverse.into(), b"dlrow", true)?
            .unwrap();
        println!("reversed: {:?}", std::str::from_utf8(&response).unwrap());
        thread::sleep(Duration::from_secs(1));
    }
}
