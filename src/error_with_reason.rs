use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
};

/// An error type that contains a reason.
#[derive(Debug)]
pub struct ErrorWithReason(pub String);

impl ErrorWithReason {
    pub fn from(string: &str) -> Self {
        ErrorWithReason(String::from(string))
    }
}

impl Display for ErrorWithReason {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ErrorWithReason {}
