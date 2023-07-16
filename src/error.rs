use crate::ffi;
use std::result::Result as StdResult;
use thiserror::Error;

// TODO: rework this error struct.
// Ideally the J* c library should return more information about the error so we can populate
// the variants with relevant data.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Syntax error encountered while parsing")]
    Syntax,
    #[error("Error encountered while compiling code")]
    Compile,
    #[error("Exception was throw while executing code")]
    Runtime,
    #[error("Error while deserializing compiled code")]
    Deserialize,
    #[error("Compiled code version mismatch")]
    Version,
}

impl TryFrom<ffi::JStarResult> for Error {
    type Error = ();
    fn try_from(value: ffi::JStarResult) -> StdResult<Self, Self::Error> {
        match value {
            ffi::JStarResult::SyntaxErr => Ok(Self::Syntax),
            ffi::JStarResult::CompileErr => Ok(Self::Compile),
            ffi::JStarResult::RuntimeErr => Ok(Self::Runtime),
            ffi::JStarResult::DeserializeErr => Ok(Self::Deserialize),
            ffi::JStarResult::VersionErr => Ok(Self::Version),
            ffi::JStarResult::Success => Err(()),
        }
    }
}

pub type Result<T> = StdResult<T, Error>;
