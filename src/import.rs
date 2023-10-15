use crate::ffi;

use std::{error::Error, ffi::CString};

/// Type representing the result of a module import.
pub type ImportResult = Result<Module, Box<dyn Error>>;

/// Represents an imported J* module.
pub enum Module {
    /// A source J* module
    Source {
        src: CString,
        path: CString,
        reg: *mut ffi::JStarNativeReg,
    },
    /// A binary J* module (bytecode)
    Binary {
        code: Vec<u8>,
        path: CString,
        reg: *mut ffi::JStarNativeReg,
    },
}

impl Module {
    /// Construct a new [Module] with J* source code.
    pub fn source(src: String, path: String) -> Self {
        Self::source_with_reg(src, path, std::ptr::null_mut())
    }

    /// Same as [source](#method.source) but with a native registry.
    pub fn source_with_reg(src: String, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        Module::Source {
            src: CString::new(src).expect("Couldn't create a c compatible string from `src`"),
            path: CString::new(path).expect("Couldn't create a c compatible string from `path`"),
            reg,
        }
    }

    /// Construct a new module with J* bytecode.
    pub fn binary(code: Vec<u8>, path: String) -> Self {
        Self::binary_with_reg(code, path, std::ptr::null_mut())
    }

    /// Same as [source](#method.binary) but with a native registry.
    pub fn binary_with_reg(code: Vec<u8>, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        let path = CString::new(path).expect("Couldn't create a c compatible string from `path`");
        Module::Binary { code, path, reg }
    }
}
