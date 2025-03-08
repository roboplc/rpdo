// TODO subscribe command
// TODO unsubscribe command
// TODO nostd
pub mod comm;
pub mod context;
mod error;
pub mod host;
pub mod io;

pub use error::Error;

pub type Result<T> = std::result::Result<T, error::Error>;

#[cfg(feature = "locking-default")]
use parking_lot::Mutex;
#[cfg(feature = "locking-rt")]
use parking_lot_rt::Mutex;
#[cfg(feature = "locking-rt-safe")]
use rtsc::pi::Mutex;
