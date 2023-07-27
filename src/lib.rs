pub use jstar_sys as ffi;
pub mod conf;
pub mod convert;
pub mod error;
pub mod import;
pub mod string;
pub mod vm;

use ffi::{JSR_CORE_MODULE, JSR_MAIN_MODULE};

pub const CORE_MODULE: &str = JSR_CORE_MODULE;
pub const MAIN_MODULE: &str = JSR_MAIN_MODULE;
