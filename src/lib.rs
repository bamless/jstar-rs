#![warn(clippy::unwrap_used)]

pub use jstar_sys as ffi;
pub mod conf;
pub mod convert;
pub mod error;
pub mod import;
pub mod native;
pub mod string;
pub mod vm;

use ffi::{JSR_CORE_MODULE, JSR_MAIN_MODULE};

/// The name of the core module.
///
/// This is the J* module that contains all the standard library and that is implicitly imported
/// by any other module. Automatically bootstrapped by the VM on runtime initialization.
pub const CORE_MODULE: &str = JSR_CORE_MODULE;

/// The name of the main module.
///
/// This is typically used as the name of the module that contains the entry point of the program.
/// The VM will automatically initialize this module during runtime initialization, so it is always
/// available.
///
/// It is notably used by [vm::VM::eval] when evaluating a script.
pub const MAIN_MODULE: &str = JSR_MAIN_MODULE;
