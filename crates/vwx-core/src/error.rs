use std::fmt;

/// Errors of the VWX reader.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// The bytes are not a Vectorworks file (or not one this reader knows).
    NotVwx(String),
    /// A Vectorworks file whose content this reader cannot decode yet.
    NotYetDecoded(String),
    /// A structure inside the file is not as expected.
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::NotVwx(why) => write!(f, "not a Vectorworks file: {why}"),
            Error::NotYetDecoded(what) => write!(f, "not decoded yet: {what}"),
            Error::Corrupt(what) => write!(f, "damaged file: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
