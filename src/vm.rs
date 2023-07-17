use crate::conf::Conf;
use crate::convert::FromJStar;
use crate::error::Error;
use crate::error::Result;
use crate::ffi::{self, jsrEvalString, jsrFreeVM, JStarConf, JStarVM};
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::path::PathBuf;

// TODO: use newly exposed jsrValidate stack and jsrValidateSlot to make wrappers memory safe

pub struct NewVM<'a> {
    vm: *mut ffi::JStarVM,
    ownership: VMOwnership<'a>,
}

impl<'a> NewVM<'a> {
    pub fn new(conf: Conf<'a>) -> Self {
        let mut trampolines = Box::new(Trampolines {
            error_callback: conf.error_callback,
            import_callback: conf.import_callback,
        });

        let conf = ffi::JStarConf {
            heap_grow_rate: conf.heap_grow_rate,
            first_gc_collection_point: conf.first_gc_collection_point,
            starting_stack_sz: conf.starting_stack_sz,
            error_callback: error_trampoline,
            import_callback: import_trampoline,
            custom_data: (&mut *trampolines as *mut _) as *mut c_void,
        };

        let vm = unsafe { ffi::jsrNewVM(&conf as *const JStarConf) };
        assert!(!vm.is_null());

        NewVM {
            vm,
            ownership: VMOwnership::Owned(trampolines),
        }
    }

    pub fn init_runtime(mut self) -> VM<'a> {
        unsafe { ffi::jsrInitRuntime(self.vm) };
        VM {
            vm: self.vm,
            ownership: std::mem::replace(&mut self.ownership, VMOwnership::NonOwned),
        }
    }
}

impl<'a> Drop for NewVM<'a> {
    fn drop(&mut self) {
        if let VMOwnership::Owned(_) = self.ownership {
            unsafe { jsrFreeVM(self.vm) };
        }
    }
}

#[non_exhaustive]
pub struct VM<'a> {
    vm: *mut JStarVM,
    ownership: VMOwnership<'a>,
}

impl<'a> VM<'a> {
    /// Construct a new `VM` wrapper starting from a raw `JStarVM` pointer.
    /// Its main use is to construct a `VM` wrapper struct across ffi boundaries when only a
    /// `JStarVM` pointer is available (for example, in J* native functions).
    ///
    /// # Safety
    ///
    /// The caller must ensure that this wrapper lives only as long as the main `VM` wrapper does.
    /// This is to ensure that the pointer to the underlying `JStarVM` and its user-defined
    /// callbacks (`Trampolines` struct) remain valid, since they will be dropped when the original
    /// `VM` wrapper lifetime ends.
    pub unsafe fn from_ptr(vm: *mut ffi::JStarVM) -> Self {
        VM {
            vm,
            ownership: VMOwnership::NonOwned,
        }
    }

    /// Evaluate J* source code
    ///
    /// # Arguments
    ///
    /// * `path` - A string representing the source code path. It doesn't have to be a
    /// real filesystem path, as it is only used during error callbacks to provide useful context
    /// to the client handling the error. Nonetheless, if the source code has been indeed read from
    /// a file, it is reccomended to pass its path to this function.
    ///
    /// * `src` - The J* source code
    pub fn eval_string(&mut self, path: &str, src: &str) -> Result<()> {
        let path = CString::new(path).unwrap();
        let src = CString::new(src).unwrap();
        let res = unsafe { jsrEvalString(self.vm, path.as_ptr(), src.as_ptr()) };
        if let Ok(err) = res.try_into() {
            Err(err)
        } else {
            Ok(())
        }
    }

    pub fn pop(&mut self) {
        unsafe { ffi::jsrPop(self.vm) };
    }

    pub fn pop_n(&mut self, n: i32) {
        assert!(n >= 0, "`n` must be greater or equal to 0");
        unsafe { ffi::jsrPopN(self.vm, n) };
    }

    pub fn push_number(&mut self, number: f64) {
        unsafe { ffi::jsrPushNumber(self.vm, number) };
    }

    pub fn is_number(&self, slot: Index) -> bool {
        unsafe { ffi::jsrIsNumber(self.vm, slot) }
    }

    pub fn get_number(&self, slot: Index) -> Option<f64> {
        if !self.is_number(slot) {
            None
        } else {
            Some(unsafe { ffi::jsrGetNumber(self.vm, slot) })
        }
    }

    pub fn get_top<'vm>(&'vm self) -> StackRef<'vm, 'a> {
        StackRef {
            index: unsafe { ffi::jsrTop(self.vm) },
            vm: self,
        }
    }
}

impl<'a> Drop for VM<'a> {
    fn drop(&mut self) {
        if let VMOwnership::Owned(_) = self.ownership {
            unsafe { jsrFreeVM(self.vm) };
        }
    }
}

pub type Index = c_int;

pub struct StackRef<'a, 'b> {
    index: Index,
    vm: &'a VM<'b>,
}

impl<'a, 'b> StackRef<'a, 'b> {
    pub fn get<T>(&self) -> Option<T>
    where
        T: FromJStar,
    {
        T::from_jstar(self.vm, self.index)
    }
}

pub type ErrorCallback<'a> = Box<dyn FnMut(Error, &str, Option<i32>, &str) + 'a>;
pub type ImportCallback<'a> = Box<dyn FnMut(&mut VM, &str) -> ImportResult + 'a>;

pub struct ImportModule {
    code: Vec<u8>,
    path: PathBuf,
    reg: *mut ffi::JStarNativeReg,
}

pub enum ImportResult {
    Success(ImportModule),
    NotFound,
}

impl ImportModule {
    pub fn new(code: Vec<u8>, path: PathBuf) -> Self {
        ImportModule {
            code,
            path,
            reg: std::ptr::null_mut(),
        }
    }

    pub fn with_reg(code: Vec<u8>, path: PathBuf, reg: *mut ffi::JStarNativeReg) -> Self {
        ImportModule { code, path, reg }
    }
}

struct Trampolines<'a> {
    error_callback: Option<ErrorCallback<'a>>,
    import_callback: Option<ImportCallback<'a>>,
}

enum VMOwnership<'a> {
    Owned(Box<Trampolines<'a>>),
    NonOwned,
}

extern "C" fn error_trampoline(
    vm: *mut ffi::JStarVM,
    err: ffi::JStarResult,
    file: *const c_char,
    line: c_int,
    error: *const c_char,
) {
    // SAFETY: jsrGetCustomData() always returns a `*const Trampolines` by construction (see
    // `NewVM::new`) and so the cast is safe.
    // Also, the Trampolines struct is guaranteed to live as long as the vm does, as it is stored
    // as an owned Box inside of it (`Owned` variant of `VMOwnership` enum). Since this function can
    // only be called during the lifetime of the vm, the dereference is safe.
    let trampolines = unsafe { &mut *(ffi::jsrGetCustomData(vm) as *mut Trampolines) };

    if let Some(ref mut error_callback) = trampolines.error_callback {
        let err = Error::try_from(err).expect("err to not be JStarResult::Success");
        let line = if line > 0 { Some(line) } else { None };
        let file = unsafe { CStr::from_ptr(file) }
            .to_str()
            .expect("file to be valid utf8");
        let error = unsafe { CStr::from_ptr(error) }
            .to_str()
            .expect("file to be valid utf8");
        error_callback(err, file, line, error);
    }
}

extern "C" fn import_trampoline(
    vm: *mut ffi::JStarVM,
    module_name: *const c_char,
) -> ffi::JStarImportResult {
    // SAFETY: ditto
    let trampolines = unsafe { &mut *(ffi::jsrGetCustomData(vm) as *mut Trampolines) };

    if let Some(ref mut import_callback) = trampolines.import_callback {
        // SAFETY: this function can only be called during the lifetime of the vm, so it is
        // guaranteed that the returned returned wrapper is safe to use
        let mut vm = unsafe { VM::from_ptr(vm) };
        let module_name = unsafe { CStr::from_ptr(module_name) }
            .to_str()
            .expect("module_name to be valid utf8");

        let _ = import_callback(&mut vm, module_name);
        ffi::JStarImportResult::default()
    } else {
        ffi::JStarImportResult::default()
    }
}
