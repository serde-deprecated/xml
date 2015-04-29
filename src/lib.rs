extern crate serde;

pub use self::de::{Deserializer, from_str};
pub use self::error::{Error, ErrorCode};

pub mod de;
pub mod error;
pub mod value;

#[cfg(not(ndebug))]
const DEBUG: bool = true;

#[cfg(ndebug)]
const DEBUG: bool = false;
