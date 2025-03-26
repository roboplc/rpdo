#![deny(missing_docs)]
#![ doc = include_str!( concat!( env!( "CARGO_MANIFEST_DIR" ), "/", "README.md" ) ) ]
// TODO subscribe command
// TODO unsubscribe command
// TODO nostd
/// Communication
pub mod comm;
/// Shared context
pub mod context;
mod error;
/// Host
pub mod host;
/// I/O helpers
pub mod io;

pub use error::Error;

/// Result type
pub type Result<T> = std::result::Result<T, error::Error>;

#[cfg(feature = "locking-default")]
use parking_lot::Mutex;
#[cfg(feature = "locking-rt")]
use parking_lot_rt::Mutex;
#[cfg(feature = "locking-rt-safe")]
use rtsc::pi::Mutex;
