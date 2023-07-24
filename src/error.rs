use crate::ffi;
use thiserror::Error;

/// Error represents a J* vm error.
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
    fn try_from(value: ffi::JStarResult) -> std::result::Result<Self, Self::Error> {
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

/// Alias for a result with an [Error]. Provided for ease of use.
pub type Result<T> = std::result::Result<T, Error>;
