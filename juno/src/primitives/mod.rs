mod interval;
mod timestamp;

pub use interval::Interval;
pub use timestamp::Timestamp;

use std::fmt;

#[derive(fmt::Debug, thiserror::Error, PartialEq, Eq)]
#[error("failed to parse input")]
pub struct ParseError;
