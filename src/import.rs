use crate::ffi;
use std::ffi::CString;

pub type ImportResult = std::result::Result<Module, ()>;

/// Represents an imported J* module.
pub enum Module {
    /// A source J* module
    Source(CString, CString, *mut ffi::JStarNativeReg),
    /// A binary J* module
    Binary(Vec<u8>, CString, *mut ffi::JStarNativeReg),
}

impl Module {
    pub fn source(src: String, path: String) -> Self {
        Self::source_with_reg(src, path, std::ptr::null_mut() as *mut ffi::JStarNativeReg)
    }

    pub fn source_with_reg(src: String, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        Module::Source(
            CString::new(src).expect("Couldn't create a c compatible string from `src`"),
            CString::new(path).expect("Couldn't create a c compatible string from `path`"),
            reg,
        )
    }

    pub fn binary(code: Vec<u8>, path: String) -> Self {
        Self::binary_with_reg(code, path, std::ptr::null_mut() as *mut ffi::JStarNativeReg)
    }

    pub fn binary_with_reg(code: Vec<u8>, path: String, reg: *mut ffi::JStarNativeReg) -> Self {
        let path = CString::new(path).expect("Couldn't create a c compatible string from `path`");
        Module::Binary(code, path, reg)
    }
}
