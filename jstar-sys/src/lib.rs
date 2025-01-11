use std::ffi::{self, c_void};
use std::os::raw::{c_char, c_int};
use std::usize;

pub enum JStarVM {}

pub type JStarNative = extern "C" fn(*mut JStarVM) -> bool;

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
#[derive(Debug, Clone, Copy)]
pub struct JStarImportResult {
    pub code: *const c_char,
    pub code_len: usize,
    pub path: *const c_char,
    pub reg: *mut JStarNativeReg,
    pub finalize: Option<JStarImportFinalizeCB>,
    pub user_data: *const c_void,
}

impl Default for JStarImportResult {
    fn default() -> Self {
        JStarImportResult {
            code: std::ptr::null() as *const c_char,
            code_len: 0,
            path: std::ptr::null() as *const c_char,
            reg: std::ptr::null_mut(),
            finalize: None,
            user_data: std::ptr::null_mut() as *const c_void,
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
#[derive(Debug, Clone, Copy)]
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

    pub fn jsrEvalModuleString(
        vm: *mut JStarVM,
        path: *const c_char,
        module: *const c_char,
        src: *const c_char,
    );

    pub fn jsrEval(
        vm: *mut JStarVM,
        path: *const c_char,
        code: *const c_void,
        len: usize,
    ) -> JStarResult;

    pub fn jsrEvalModule(
        vm: *mut JStarVM,
        path: *const c_char,
        module: *const c_char,
        code: *const c_void,
        len: usize,
    ) -> JStarResult;

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
    ) -> bool;
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
// EXCEPTION API
// -----------------------------------------------------------------------------

extern "C" {
    pub fn jsrRaise(vm: *mut JStarVM, cls: *const c_char, err: *const c_char, ...);
}

// -----------------------------------------------------------------------------
// MODULE API
// -----------------------------------------------------------------------------

extern "C" {
    pub fn jsrSetGlobal(vm: *mut JStarVM, module: *const c_char, name: *const c_char) -> bool;
    pub fn jsrGetGlobal(vm: *mut JStarVM, module: *const c_char, name: *const c_char) -> bool;
}

// -----------------------------------------------------------------------------
// TYPE CHECKING FUNCTIONS
// -----------------------------------------------------------------------------

extern "C" {
    pub fn jsrIsNumber(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsInteger(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsString(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsList(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsTuple(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsBoolean(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsHandle(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsNull(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsInstance(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsTable(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsFunction(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrIsUserdata(vm: *mut JStarVM, slot: c_int) -> bool;

    pub fn jsrCheckNumber(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckInt(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckString(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckList(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckTuple(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckBoolean(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckNull(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckInstance(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckHandle(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckTable(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckFunction(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
    pub fn jsrCheckUserdata(vm: *mut JStarVM, slot: c_int, name: *const c_char) -> bool;
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
    pub fn jsrValidateSlot(vm: *mut JStarVM, slot: c_int) -> bool;
    pub fn jsrValidateStack(vm: *mut JStarVM) -> bool;
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
#[derive(Debug, Clone, Copy)]
pub struct JStarRegMethod {
    cls: *const c_char,
    name: *const c_char,
    meth: JStarNative,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
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

extern "C" {
    pub fn jsrCompileCode(
        vm: *mut JStarVM,
        path: *const c_char,
        src: *const c_char,
        out: *mut JStarBuffer,
    ) -> JStarResult;

    pub fn jsrDisassembleCode(
        vm: *mut JStarVM,
        path: *const c_char,
        code: *const JStarBuffer,
    ) -> JStarResult;
}

// omitted: jsrReadFile

// -----------------------------------------------------------------------------
// Buffer
// -----------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug)]
pub struct JStarBuffer {
    pub vm: *mut JStarVM,
    pub capacity: usize,
    pub size: usize,
    pub data: *mut c_char,
}

impl Default for JStarBuffer {
    fn default() -> Self {
        JStarBuffer {
            vm: std::ptr::null_mut(),
            capacity: 0,
            size: 0,
            data: std::ptr::null_mut(),
        }
    }
}

extern "C" {
    pub fn jsrBufferPush(b: *mut JStarBuffer);
    pub fn jsrBufferFree(b: *mut JStarBuffer);
}

// omitted: JStarBuffer API
