#![allow(dead_code)]

use std::{backtrace, convert::Infallible};
pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("{msg}")]
    Backtraced {
        msg: String,
        backtrace: backtrace::Backtrace,
    },
    #[error(transparent)]
    FromInt(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    IntParse(#[from] std::num::ParseIntError),
    #[error(transparent)]
    RocksDB(#[from] rust_rocksdb::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("str utf8: {0}")]
    StrUtf8(#[from] std::str::Utf8Error, std::backtrace::Backtrace),
    #[error("string utf8: {0}")]
    StringUtf8(
        #[from] std::string::FromUtf8Error,
        std::backtrace::Backtrace,
    ),
    #[error("io err: {0}")]
    Io(#[from] std::io::Error, std::backtrace::Backtrace),
    #[error("sqlite error: {0}")]
    SQLite(#[from] rusqlite::Error, std::backtrace::Backtrace),
    #[error("clap error: {0}")]
    Clap(#[from] clap::Error),
    #[error(transparent)]
    Anyhow(
        #[from]
        #[backtrace]
        anyhow::Error,
    ),
}

impl Error {
    pub fn backtraced(msg: &str) -> Self {
        Error::Backtraced {
            msg: msg.to_owned(),
            backtrace: backtrace::Backtrace::capture(),
        }
    }

    pub fn none_err() -> Self {
        Error::backtraced("Some option expected, got none")
    }
}

impl std::convert::From<String> for Error {
    fn from(value: String) -> Self {
        Error::Message(value)
    }
}

impl std::convert::From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        From::<String>::from(value.to_string())
    }
}

impl std::convert::From<std::borrow::Cow<'_, str>> for Error {
    fn from(value: std::borrow::Cow<'_, str>) -> Self {
        From::<String>::from(value.into_owned())
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub trait OptionExt<T> {
    fn ok_or_err(self) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_err(self) -> Result<T> {
        match self {
            Some(v) => Ok(v),
            None => Err(Error::none_err()),
        }
    }
}

pub trait ResultExt<T> {
    fn ext(self) -> Result<T>;
}

impl<T, E: Into<Error>> ResultExt<T> for std::result::Result<T, E> {
    fn ext(self) -> Result<T> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(e.into()),
        }
    }
}
