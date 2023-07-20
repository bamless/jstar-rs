use ffi::jsrGetString;
use ffi::jsrGetStringSz;

use crate::conf::Conf;
use crate::convert::FromJStar;
use crate::error::Error;
use crate::error::Result;
use crate::ffi::{self, jsrEvalString, jsrFreeVM, JStarConf, JStarVM};
use crate::string::String as JStarString;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

pub type ErrorCallback<'a> = Box<dyn FnMut(Error, &str, Option<i32>, &str) + 'a>;
pub type ImportCallback<'a> = Box<dyn FnMut(&mut VM, &str) -> ImportResult + 'a>;

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
    /// Construct a new [VM] wrapper starting from a raw [JStarVM] pointer.
    /// Its main use is to construct a `VM` wrapper struct across ffi boundaries when only a
    /// `JStarVM` pointer is available (for example, in J* native functions).
    ///
    /// # Safety
    ///
    /// The caller must ensure that this wrapper lives only as long as the main `VM` wrapper does.
    /// This is to ensure that the pointer to the underlying `JStarVM` and its user-defined
    /// callbacks ([Trampolines] struct) remain valid, since they will be dropped when the original
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
    pub fn eval_string(&self, path: &str, src: &str) -> Result<()> {
        let path = CString::new(path).expect("Couldn't create CString");
        let src = CString::new(src).expect("Couldn't create CString");
        let res = unsafe { jsrEvalString(self.vm, path.as_ptr(), src.as_ptr()) };
        if let Ok(err) = res.try_into() {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Pops one element from the VM stack.
    /// This method panics if we try to pop more items than the stack holds.
    pub fn pop(&mut self) {
        assert!(self.validate_slot(-1), "VM stack underflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPop(self.vm) };
    }

    /// Pops `n` elements from the VM stack
    /// This method panics if we try to pop more items than the stack holds.
    pub fn pop_n(&mut self, n: i32) {
        assert!(n > 0, "`n` must be greater than 0");
        assert!(self.validate_slot(-n), "VM stack underflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPopN(self.vm, n) };
    }

    /// Push a `Number` onto the VM stack.
    /// This method panics if there isn't enough stack space for one element.
    /// Use [ensure_stack](#method.ensure_stack) if you are not sure the stack has enough space.
    pub fn push_number(&self, number: f64) {
        assert!(self.validate_stack(), "VM stack overflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPushNumber(self.vm, number) };
    }

    /// Returns wether or not the value at `slot` is a `Number`.
    pub fn is_number(&self, slot: Index) -> bool {
        assert!(self.validate_slot(slot), "VM stack overflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrIsNumber(self.vm, slot) }
    }

    pub fn get_number(&self, slot: Index) -> Option<f64> {
        if !self.is_number(slot) {
            None
        } else {
            // SAFETY: `slot` is a valide slot per check above, and its a `Number`
            Some(unsafe { ffi::jsrGetNumber(self.vm, slot) })
        }
    }

    pub fn push_string(&self, str: impl AsRef<[u8]>) {
        let str = str.as_ref();
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPushStringSz(self.vm, str.as_ptr() as *const c_char, str.len()) }
    }

    pub fn is_string(&self, slot: Index) -> bool {
        assert!(self.validate_slot(slot), "Invalid slot");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrIsString(self.vm, slot) }
    }

    pub fn get_string(&self, slot: Index) -> Option<JStarString> {
        if !self.is_string(slot) {
            None
        } else {
            // SAFETY: `slot` is a valide slot per check above, and its a `Number`
            let data = unsafe { jsrGetString(self.vm, slot) };
            let len = unsafe { jsrGetStringSz(self.vm, slot) };
            Some(JStarString::new(data, len))
        }
    }

    pub fn get_top(&self) -> StackRef {
        StackRef {
            // SAFETY: `self.vm` is a valid J* vm pointer
            index: unsafe { ffi::jsrTop(self.vm) },
            vm: self,
        }
    }

    /// Ensure that the vm's stack can hold at least `needed` items, reallocating the stack
    /// to add more space if needed.
    pub fn ensure_stack(&self, needed: usize) {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrEnsureStack(self.vm, needed) };
    }

    /// Returns `true` if the provided slot is valid, i.e. it doesn't overflow or underflow the native
    /// stack, false otherwise
    pub fn validate_slot(&self, slot: Index) -> bool {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrValidateSlot(self.vm, slot) }
    }

    /// Returns `true` if the stack has space for one element, i.e. pushing one element will not overflow
    /// the native stack
    pub fn validate_stack(&self) -> bool {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrValidateStack(self.vm) }
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

pub struct StackRef<'vm> {
    index: Index,
    vm: &'vm VM<'vm>,
}

impl<'vm> StackRef<'vm> {
    pub fn get<T>(&self) -> Option<T>
    where
        T: FromJStar<'vm>,
    {
        T::from_jstar(self.vm, self.index)
    }
}

pub enum Module {
    Source(CString, CString, *mut ffi::JStarNativeReg),
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

pub enum ImportResult {
    Success(Module),
    Error,
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
        let err = Error::try_from(err).expect("err shouldn't be JStarResult::Success");
        let line = if line > 0 { Some(line) } else { None };

        // SAFETY: `file` comes from the J* API that guarantess that is a valid cstring and utf8
        let file = unsafe { CStr::from_ptr(file) }
            .to_str()
            .expect("file should be valid utf8");

        // SAFETY: `error` comes from the J* API that guarantess that is a valid cstring and utf8
        let error = unsafe { CStr::from_ptr(error) }
            .to_str()
            .expect("error should be valid utf8");

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

        // SAFETY: `module_name` comes from the J* API that guarantess that is a valid cstring and utf8
        let module_name = unsafe { CStr::from_ptr(module_name) }
            .to_str()
            .expect("module_name is not valid utf8");

        match import_callback(&mut vm, module_name) {
            ImportResult::Error => ffi::JStarImportResult::default(),
            ImportResult::Success(module) => {
                let (code, path, reg) = match module {
                    Module::Source(src, path, reg) => (src.into(), path, reg),
                    Module::Binary(code, path, reg) => (code, path, reg),
                };

                struct ImportData(Vec<u8>, CString);
                let import_data = Box::new(ImportData(code, path));

                // Callback function that drops data allocated during `import_callback`
                extern "C" fn finalize_import(user_data: *mut c_void) {
                    // SAFETY: user_data is a `*mut ImportData` obtained from a Box, so it is safe
                    // to construct a new `Box` from it
                    let _ = unsafe { Box::from_raw(user_data as *mut ImportData) };
                }

                ffi::JStarImportResult {
                    code: import_data.0.as_ptr() as *const c_char,
                    code_len: import_data.0.len(),
                    path: import_data.1.as_ptr(),
                    reg,
                    finalize: Some(finalize_import),
                    user_data: Box::into_raw(import_data) as *mut _ as *mut c_void,
                }
            }
        }
    } else {
        ffi::JStarImportResult::default()
    }
}
