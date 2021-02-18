use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
};

#[derive(Debug)]
pub enum Error {
    EnumModuleError(u32),
    MissingGaError,
    ReadError(u32, usize, &'static str),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::EnumModuleError(code) => f.write_fmt(format_args!(
                "an error occurred enumerating game's modules: {}",
                code
            )),
            Error::MissingGaError => f.write_str("failed to locate GameAssembly.dll"),
            Error::ReadError(code, bytes, message) => f.write_fmt(format_args!(
                "an error occurred reading {}: read {} bytes, error code: {}",
                message, bytes, code
            )),
        }
    }
}

impl StdError for Error {}
