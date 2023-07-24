use crate::ffi;
use std::{ffi::CString, error::Error};
use thiserror::Error;

/// Type representing the result of a module import.
pub type ImportResult = Result<Module, Box<dyn Error>>;

/// Error signaling that a module couldn't be found during import resolution
#[derive(Error, Debug)]
#[error("Couldn't find module")]
pub struct NotFound;

/// Represents an imported J* module.
pub enum Module {
    /// A source J* module
    Source(CString, CString, *mut ffi::JStarNativeReg),
    /// A binary J* module (bytecode)
    Binary(Vec<u8>, CString, *mut ffi::JStarNativeReg),
}

impl Module {
    /// Construct a new [Module] with J* source code.
    pub fn source(src: String, path: String) -> Self {
        Self::source_with_reg(src, path, std::ptr::null_mut() as *mut ffi::JStarNativeReg)
    }

    /// Same as [source](#method.source) but with a native registry.
    pub fn source_with_reg(src: String, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        Module::Source(
            CString::new(src).expect("Couldn't create a c compatible string from `src`"),
            CString::new(path).expect("Couldn't create a c compatible string from `path`"),
            reg,
        )
    }

    /// Construct a new module with J* bytecode.
    pub fn binary(code: Vec<u8>, path: String) -> Self {
        Self::binary_with_reg(code, path, std::ptr::null_mut() as *mut ffi::JStarNativeReg)
    }

    /// Same as [source](#method.binary) but with a native registry.
    pub fn binary_with_reg(code: Vec<u8>, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        let path = CString::new(path).expect("Couldn't create a c compatible string from `path`");
        Module::Binary(code, path, reg)
    }
}
