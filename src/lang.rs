#![allow(dead_code)]

use std::{convert::Infallible, num::ParseFloatError};
pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Message(String, std::backtrace::Backtrace),
    #[error("try from int: {0}")]
    FromInt(#[from] std::num::TryFromIntError, std::backtrace::Backtrace),
    #[error("parse int: {0}")]
    IntParse(#[from] std::num::ParseIntError, std::backtrace::Backtrace),
    #[error("serde json: {0}")]
    Serde(#[from] serde_json::Error, std::backtrace::Backtrace),
    #[error("str utf8: {0}")]
    StrUtf8(#[from] std::str::Utf8Error, std::backtrace::Backtrace),
    #[error("parse float: {0}")]
    ParseFloat(#[from] ParseFloatError, std::backtrace::Backtrace),
    #[error("string utf8: {0}")]
    StringUtf8(
        #[from] std::string::FromUtf8Error,
        std::backtrace::Backtrace,
    ),
    #[error("io err: {0}")]
    Io(#[from] std::io::Error, std::backtrace::Backtrace),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error, std::backtrace::Backtrace),
    #[error("clap error: {0}")]
    Clap(#[from] clap::Error, std::backtrace::Backtrace),
    #[cfg(feature = "legacy-rocks")]
    #[error("rocksdb: {0}")]
    RocksDB(#[from] rust_rocksdb::Error, std::backtrace::Backtrace),
    #[error(transparent)]
    Anyhow(
        #[from]
        #[backtrace]
        anyhow::Error,
    ),
}

impl Error {
    pub fn none_err() -> Self {
        Error::from("Some option expected, got none")
    }
}

impl std::convert::From<String> for Error {
    fn from(value: String) -> Self {
        Error::Message(value, std::backtrace::Backtrace::capture())
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
