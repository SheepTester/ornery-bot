use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
};

/// An error type that contains a reason.
#[derive(Debug)]
pub struct ErrorWithReason<'a>(pub &'a str);

impl<'a> Display for ErrorWithReason<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}", self)
    }
}

impl<'a> Error for ErrorWithReason<'a> {}
