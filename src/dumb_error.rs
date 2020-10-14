#[derive(Debug)]
pub enum DumbError {
    SerenityError(serenity::prelude::SerenityError),
    IOError(std::io::Error),
    SerdeError(serde_json::error::Error),
    ReqwestError(reqwest::Error),
    CsvError(csv::Error),
}

impl From<serenity::prelude::SerenityError> for DumbError {
    fn from(error: serenity::prelude::SerenityError) -> Self {
        DumbError::SerenityError(error)
    }
}

impl From<std::io::Error> for DumbError {
    fn from(error: std::io::Error) -> Self {
        DumbError::IOError(error)
    }
}

impl From<serde_json::error::Error> for DumbError {
    fn from(error: serde_json::error::Error) -> Self {
        DumbError::SerdeError(error)
    }
}

impl From<reqwest::Error> for DumbError {
    fn from(error: reqwest::Error) -> Self {
        DumbError::ReqwestError(error)
    }
}

impl From<csv::Error> for DumbError {
    fn from(error: csv::Error) -> Self {
        DumbError::CsvError(error)
    }
}

pub type MaybeError = Result<(), DumbError>;
