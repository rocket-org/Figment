//! [`Value`] and friends: types representing valid configuration values.
//!
mod de;
mod ser;
mod tag;
mod value;

#[cfg(feature = "parse-value")]
mod parse;

#[cfg(feature = "parse-value")]
mod escape;

pub mod magic;

pub use tag::Tag;
pub use uncased::{Uncased, UncasedStr};
pub use value::{Dict, Empty, Map, Num, Value};
pub(crate) use {self::de::*, self::ser::*};
