use crate::ffi;

use thiserror::Error;

/// Alias for a result with an [enum@Error]. Provided for ease of use.
pub type Result<T> = std::result::Result<T, Error>;

/// Error represents a J* vm error.
#[derive(Error, Debug)]
pub enum Error {
    /// A syntax error was encountered while parsing
    #[error("Syntax error encountered while parsing")]
    Syntax,
    /// An error was encountered while compiling code
    #[error("Error encountered while compiling code")]
    Compile,
    /// An exception was thrown while executing code
    #[error("Exception was throw while executing code")]
    Runtime,
    /// An error was encountered while deserializing compiled code
    #[error("Error while deserializing compiled code")]
    Deserialize,
    /// Compiled code version mismatch
    #[error("Compiled code version mismatch")]
    Version,
    /// I/O error
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error)
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
