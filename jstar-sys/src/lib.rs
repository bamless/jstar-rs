use std::ffi::{self, c_void};
use std::os::raw::{c_char, c_int};

pub enum JStarVM {}

type JStarNative = extern "C" fn(*mut JStarVM) -> c_int;

#[repr(C)]
pub enum JStarResult {
    Success,
    SyntaxErr,
    CompileErr,
    RuntimeErr,
    DeserializeErr,
    VersionErr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JStarImportResult {
    code: *const c_char,
    code_len: usize,
    path: *const c_char,
    reg: *const c_void,
    finalize: Option<JStarImportFinalizeCB>,
    user_data: *mut c_void,
}

impl Default for JStarImportResult {
    fn default() -> Self {
        JStarImportResult {
            code: std::ptr::null() as *const c_char,
            code_len: 0,
            path: std::ptr::null() as *const c_char,
            reg: std::ptr::null() as *const c_void,
            finalize: None,
            user_data: std::ptr::null_mut() as *mut c_void,
        }
    }
}

// -----------------------------------------------------------------------------
// HOOKS AND CALLBAKCS
// -----------------------------------------------------------------------------

pub type JStarImportCB =
    extern "C" fn(vm: *mut JStarVM, module_name: *const c_char) -> JStarImportResult;

pub type JStarErrorCB = extern "C" fn(
    vm: *mut JStarVM,
    err: JStarResult,
    file: *const c_char,
    line: c_int,
    error: *const c_char,
) -> ();

pub type JStarImportFinalizeCB = extern "C" fn(user_data: *mut c_void) -> ();

// omitted: jsrPrintErrorCB

// -----------------------------------------------------------------------------
// J* VM INITIALIZATION
// -----------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JStarConf {
    pub starting_stack_sz: usize,
    pub first_gc_collection_point: usize,
    pub heap_grow_rate: c_int,
    pub error_callback: JStarErrorCB,
    pub import_callback: JStarImportCB,
    pub custom_data: *mut ffi::c_void,
}

impl Default for JStarConf {
    fn default() -> Self {
        unsafe { jsrGetConf() }
    }
}

extern "C" {
    pub fn jsrGetConf() -> JStarConf;
    pub fn jsrNewVM(conf: *const JStarConf) -> *mut JStarVM;
    pub fn jsrInitRuntime(vm: *mut JStarVM);
    pub fn jsrFreeVM(vm: *mut JStarVM);
    pub fn jsrInitCommandLineArgs(vm: *mut JStarVM, argc: c_int, argv: *mut *const c_char);
    pub fn jsrGetCustomData(vm: *mut JStarVM) -> *mut c_void;
    pub fn jsrEvalBreak(vm: *mut JStarVM);
}

// -----------------------------------------------------------------------------
// CODE EXECUTION
// -----------------------------------------------------------------------------

extern "C" {
    pub fn jsrEvalString(vm: *mut JStarVM, path: *const c_char, src: *const c_char) -> JStarResult;
    pub fn jsrEvalStringModule(
        vm: *mut JStarVM,
        path: *const c_char,
        module: *const c_char,
        src: *const c_char,
    );
    pub fn jsrCall(vm: *mut JStarVM, argc: u8) -> JStarResult;
    pub fn jsrCallMethod(vm: *mut JStarVM, name: *const c_char, argc: u8);
}

// -----------------------------------------------------------------------------
// C TO J* VALUE CONVERSION API
// -----------------------------------------------------------------------------

type UserdataFinalizeCB = extern "C" fn(*mut c_void);

extern "C" {
    pub fn jsrPushNumber(vm: *mut JStarVM, number: f64);
    pub fn jsrPushBoolean(vm: *mut JStarVM, boolean: bool);
    pub fn jsrPushStringSz(vm: *mut JStarVM, string: *const c_char, size: usize);
    pub fn jsrPushString(vm: *mut JStarVM, string: *const c_char);
    pub fn jsrPushHandle(vm: *mut JStarVM, handle: *mut c_void);
    pub fn jsrPushNull(vm: *mut JStarVM);
    pub fn jsrPushList(vm: *mut JStarVM);
    pub fn jsrPushTuple(vm: *mut JStarVM, size: usize);
    pub fn jsrPushTable(vm: *mut JStarVM);
    pub fn jsrPushValue(vm: *mut JStarVM, slot: c_int);
    pub fn jsrPushUserdata(vm: *mut JStarVM, size: usize, finalize: UserdataFinalizeCB);
    pub fn jsrPushNative(
        vm: *mut JStarVM,
        module: *const c_char,
        name: *const c_char,
        nat: JStarNative,
        argc: u8,
    );
    pub fn jsrPop(vm: *mut JStarVM);
    pub fn jsrPopN(vm: *mut JStarVM, n: c_int);
    pub fn jsrTop(vm: *mut JStarVM) -> c_int;
}

#[allow(non_snake_case)]
#[allow(clippy::missing_safety_doc)]
pub unsafe fn jsrDup(vm: *mut JStarVM) {
    jsrPushValue(vm, -1);
}

// -----------------------------------------------------------------------------
// J* TO C VALUE CONVERSION API
// -----------------------------------------------------------------------------

extern "C" {
    pub fn jsrGetNumber(vm: *mut JStarVM, slot: c_int) -> f64;
    pub fn jsrGetBoolean(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrGetHandle(vm: *mut JStarVM, slot: c_int) -> *mut c_void;
    pub fn jsrGetStringSz(vm: *mut JStarVM, slot: c_int) -> usize;
    pub fn jsrGetString(vm: *mut JStarVM, slot: c_int) -> *const c_char;
}

// -----------------------------------------------------------------------------
// NATIVES AND NATIVE REGISTRATION
// -----------------------------------------------------------------------------

pub const JSR_CONSTRUCT: &str = "@construct";
pub const JSR_MAIN_MODULE: &str = "__main__";
pub const JSR_CORE_MODULE: &str = "__core__";

pub const JSTAR_MIN_NATIVE_STACK_SZ: usize = 20;

// TODO: write rust macros in place of these?
// omitted: JSR_NATIVE
// omitted: JSR_RAISE

extern "C" {
    pub fn jsrEnsureStack(vm: *mut JStarVM, needed: usize);
}

#[repr(C)]
pub enum JStarRegEntryType {
    Method,
    Function,
    Sentinel,
}

#[repr(C)]
pub union JStarRegEntry {
    method: JStarRegMethod,
    function: JStarRegFunction,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JStarRegMethod {
    cls: *const c_char,
    name: *const c_char,
    meth: JStarNative,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JStarRegFunction {
    name: *const c_char,
    fun: JStarNative,
}

#[repr(C)]
pub struct JStarNativeReg {
    kind: JStarRegEntryType,
    un: JStarRegEntry,
}

// -----------------------------------------------------------------------------
// CODE COMPILATION
// -----------------------------------------------------------------------------

// TODO
