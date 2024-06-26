use crate::conf::Conf;
use crate::conf::ErrorCallback;
use crate::conf::ImportCallback;
use crate::convert::FromJStar;
use crate::error::Error;
use crate::error::Result;
use crate::ffi;
use crate::import::Module;
use crate::string::String as JStarString;

use std::ffi::CStr;
use std::ffi::CString;
use std::io::Write;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_void};
use std::slice::from_raw_parts;

/// Type representing an offset into the J* stack.
/// If positive it represents a position from the start of the stack, if negative from its end.
pub type Index = c_int;

/// Marker struct that represents an uninitialized vm.
/// An uninitialized vm doesn't have a language runtime yet, so it can only perform operations that
/// don't require one, such as compiling J* code or allocating vm-managed buffers.
/// To obtain a fully initialized vm that can execute code call the [VM::init_runtime] method.
/// Keep in mind that initializing the runtime *will* execute J* code and allocate memory and as
/// such it's a (relatively) slow process. Only call the initialization when needed and outside
/// performance critical sections.
pub struct Uninit;

/// Marker struct that represents a fully initialized J* vm.
/// Capable of executing J* code, as well as performing any operations an [Uninit] vm can.
pub struct Init;

/// A J* virtual machine.
/// This is the main entry point of the J* API.
pub struct VM<'a, State = Init> {
    vm: *mut ffi::JStarVM,
    ownership: VMOwnership<'a>,
    state: PhantomData<State>,
}

impl<'a, State> Drop for VM<'a, State> {
    fn drop(&mut self) {
        if let VMOwnership::Owned(_trampolines) = &self.ownership {
            unsafe { ffi::jsrFreeVM(self.vm) };
        }
    }
}

impl<'a> VM<'a, Uninit> {
    /// Constructs a new J* vm configured with the settings specified in [Conf].
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

        let vm = unsafe { ffi::jsrNewVM(&conf as *const ffi::JStarConf) };
        assert!(!vm.is_null());

        VM {
            vm,
            ownership: VMOwnership::Owned(trampolines),
            state: PhantomData,
        }
    }

    /// Initializes the J* runtime.
    /// After calliing this method the returned [VM] will be capable of executing J* code.
    pub fn init_runtime(mut self) -> VM<'a, Init> {
        // SAFETY: `self.vm` is a valid pointer
        unsafe { ffi::jsrInitRuntime(self.vm) };
        VM {
            vm: self.vm,
            ownership: std::mem::replace(&mut self.ownership, VMOwnership::NonOwned),
            state: PhantomData,
        }
    }
}

impl<'a> VM<'a, Init> {
    /// Construct a new [VM] wrapper starting from a raw [ffi::JStarVM] pointer.
    /// Its main use is to construct a `VM` wrapper struct across ffi boundaries when only a
    /// `JStarVM` pointer is available (for example, in J* native functions).
    ///
    /// # Safety
    ///
    /// The caller must ensure that this wrapper lives only as long as the main `VM` wrapper does.
    /// This is to ensure that the pointer to the underlying `JStarVM` and its user-defined
    /// callbacks remain valid, since they will be dropped when the original `VM` wrapper lifetime
    /// ends.
    pub unsafe fn from_ptr(vm: *mut ffi::JStarVM) -> Self {
        VM {
            vm,
            ownership: VMOwnership::NonOwned,
            state: PhantomData,
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
        let path = CString::new(path).expect("Couldn't create CString");
        let src = CString::new(src).expect("Couldn't create CString");
        let res = unsafe { ffi::jsrEvalString(self.vm, path.as_ptr(), src.as_ptr()) };
        if let Ok(err) = res.try_into() {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Similar to [VM::eval_string] but takes in an arbitrary [u8] slice, so that it can also
    /// avaluate compiled J* code.
    pub fn eval(&mut self, path: &str, code: impl AsRef<[u8]>) -> Result<()> {
        let path = CString::new(path).expect("Couldn't create CString");
        let code = code.as_ref();
        let res = unsafe {
            ffi::jsrEval(
                self.vm,
                path.as_ptr(),
                code.as_ptr() as *const c_void,
                code.len(),
            )
        };
        if let Ok(err) = res.try_into() {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Call the value at slot `-(argc - 1)` with the arguments from `-argc..$top`.
    ///
    /// # Returns
    ///
    /// Ok(()) if the call succeded leaving the result on top of the stack, Err([Error::Runtime]) if
    /// the the call failed leaving an Exception on top of the stack.
    /// In both cases, the args and the callee are popped from the stack.
    pub fn call(&mut self, argc: u8) -> Result<()> {
        assert!(self.validate_slot(-(argc as i32 + 1)));
        // SAFETY: `self.vm` is a valid pointer
        let res = unsafe { ffi::jsrCall(self.vm, argc) };
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
    /// Use [VM::ensure_stack] if you are not sure the stack has enough space.
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

    /// Gets a J* `Number` from the stack.
    /// If the value at `slot` is not a `Number`, returns `None`.
    pub fn get_number(&self, slot: Index) -> Option<f64> {
        if !self.is_number(slot) {
            None
        } else {
            // SAFETY: `slot` is a valide slot per check above, and its a `Number`
            Some(unsafe { ffi::jsrGetNumber(self.vm, slot) })
        }
    }

    /// Gets a J* `Number` from the stack, checking that it is a `Number` and leaving a
    /// `TypeException` on the stack if it is not.
    pub fn check_number(&self, slot: Index, name: &str) -> Result<f64> {
        assert!(self.validate_slot(slot), "VM stack overflow");
        let name = CString::new(name).expect("Error converting `name` to c-string");
        if !unsafe { ffi::jsrCheckNumber(self.vm, slot, name.as_ptr()) } {
            Err(Error::Runtime)
        } else {
            Ok(unsafe { ffi::jsrGetNumber(self.vm, slot) })
        }
    }

    /// Push a `String` onto the VM stack.
    /// Since a J* string can contain arbitrary bytes, this method accepts anything that can be
    /// trated as a byte slice.
    /// This method panics if there isn't enough stack space for one element.
    /// Use [VM::ensure_stack] if you are not sure the stack has enough space.
    pub fn push_string(&self, str: impl AsRef<[u8]>) {
        let str = str.as_ref();
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPushStringSz(self.vm, str.as_ptr() as *const c_char, str.len()) }
    }

    /// Returns wether or not the value at `slot` is a `String`.
    pub fn is_string(&self, slot: Index) -> bool {
        assert!(self.validate_slot(slot), "`slot` out of bounds");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrIsString(self.vm, slot) }
    }

    /// Gets a J* `String` from the stack.
    /// If the value at `slot` is not a `String`, returns `None`.
    pub fn get_string(&self, slot: Index) -> Option<JStarString> {
        if !self.is_string(slot) {
            None
        } else {
            // SAFETY: `slot` is a valid slot per check above, and its a `Number`
            let data = unsafe { ffi::jsrGetString(self.vm, slot) };
            let len = unsafe { ffi::jsrGetStringSz(self.vm, slot) };
            Some(JStarString::new(data, len))
        }
    }

    /// Gets a J* `String` from the stack, checking that it is a `String` and leaving a
    /// `TypeException` on top of the stack if it is not.
    pub fn check_string(&self, slot: Index, name: &str) -> Result<JStarString> {
        assert!(self.validate_slot(slot), "VM stack overflow");
        let name = CString::new(name).expect("Error converting `name` to c-string");
        if !unsafe { ffi::jsrCheckString(self.vm, slot, name.as_ptr()) } {
            Err(Error::Runtime)
        } else {
            let data = unsafe { ffi::jsrGetString(self.vm, slot) };
            let len = unsafe { ffi::jsrGetStringSz(self.vm, slot) };
            Ok(JStarString::new(data, len))
        }
    }

    /// Get a global variable `name` from module `module_name`.
    ///
    /// # Returns
    ///
    /// Ok(()) in case of success leaving the value on top of the stack, [Err(Error::Runtime)] in
    /// case of failure leaving an exception on top of the stack.
    pub fn get_global(&self, module_name: &str, name: &str) -> Result<()> {
        // TODO: check that `module_name` exists. New J* apis should be added for this.
        assert!(self.validate_stack());
        let module_name =
            CString::new(module_name).expect("Error converting `module` name to c-string");
        let name = CString::new(name).expect("Error converting `name` to c-string");
        let res = unsafe { ffi::jsrGetGlobal(self.vm, module_name.as_ptr(), name.as_ptr()) };
        if !res {
            Err(Error::Runtime)
        } else {
            Ok(())
        }
    }

    /// Sets a global variable `name` in module `module_name` with the value on top of the stack.
    /// The value is not popped.
    pub fn set_global(&self, module_name: &str, name: &str) {
        // TODO: check that `module_name` exists. New J* apis should be added for this.
        assert!(self.validate_slot(-1));
        let module_name =
            CString::new(module_name).expect("Error converting `module` name to c-string");
        let name = CString::new(name).expect("Error converting `name` to c-string");
        unsafe { ffi::jsrSetGlobal(self.vm, module_name.as_ptr(), name.as_ptr()) };
    }

    /// Pushes a naive function onto the stack.
    ///
    /// See [crate::native] for utility functions and macros to create natives.
    ///
    /// # Arguments
    ///
    /// * `module` - The name of the module the function belongs to
    /// * `name` - The name of the function
    /// * `func` - The native function to push
    /// * `argc` - The number of arguments the function takes
    pub fn push_native(&self, module: &str, name: &str, func: ffi::JStarNative, argc: u8) {
        let module = CString::new(module).expect("`module` to be a valid CString");
        let name = CString::new(name).expect("`name` to be a valid CString");
        unsafe { ffi::jsrPushNative(self.vm, module.as_ptr(), name.as_ptr(), func, argc) };
    }

    /// Registers a native function in the global scope of module `module`.
    ///
    /// This is a convenience method that is equivalent to doing:
    /// ```ignore
    /// vm.push_native(module, "func_name", func, argc);
    /// vm.set_global(module, "func_name");
    /// vm.pop();
    /// ```
    ///
    /// # Arguments
    ///
    /// * `module` - The name of the module the function belongs to
    /// * `name` - The name the function will be bound to
    /// * `func` - The native function to register
    /// * `argc` - The number of arguments the function takes
    pub fn register_native(&mut self, module: &str, name: &str, func: ffi::JStarNative, argc: u8) {
        self.push_native(module, name, func, argc);
        self.set_global(module, name);
        self.pop();
    }

    /// Returns a [StackRef] pointing to the topmost stack slot.
    pub fn get_top(&self) -> StackRef {
        StackRef {
            // SAFETY: `self.vm` is a valid J* vm pointer
            index: unsafe { ffi::jsrTop(self.vm) },
            vm: self,
        }
    }

    /// Returns a [StackRef] pointing to the stack slot at `slot`.
    /// `slot` is treated as an offset from the top of the stack and must be positive.
    /// This method panics if `slot` is out of bounds.
    pub fn peek_top(&self, slot: Index) -> StackRef {
        assert!(slot > 0, "`slot` must be positive");
        // SAFETY: `self.vm` is a valid J* vm pointer
        let idx = unsafe { ffi::jsrTop(self.vm) } - slot;
        assert!(self.validate_slot(idx), "`slot` out of bounds");
        StackRef {
            index: idx,
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

impl<'a, State> VM<'a, State> {
    /// Compiles J* source code into bytecode.
    ///
    /// # Arguments
    ///
    /// * `src` - The J* source code to compile
    ///
    /// * `path` - The path of the source code. It doesn't have to be a real filesystem path, as it
    /// is only used during error callbacks to provide useful context to the client handling the
    /// error. Nonetheless, if the source code has been indeed read from a file, it is reccomended
    /// to pass its path to this function.
    ///
    /// * `out` - A [Write] implementor to write the compiled bytecode to
    ///
    /// # Returns
    ///
    /// Ok(()) if the compilation succeded, Err([Error]) otherwise.
    pub fn compile(&self, path: &str, src: &str, mut out: impl Write) -> Result<()> {
        let path = CString::new(path).expect("`path` to not contain NUL characters");
        let src = CString::new(src).expect("`src` to not contain NUL characters");
        let mut buf = ffi::JStarBuffer::default();

        // SAFETY: `self.vm` is a valid pointer
        let res = unsafe {
            ffi::jsrCompileCode(
                self.vm,
                path.as_ptr(),
                src.as_ptr(),
                &mut buf as *mut ffi::JStarBuffer,
            )
        };

        if let Ok(err) = res.try_into() {
            return Err(err);
        }

        // SAFETY: we are guaranteed by the J* API that `buf.data` is a valid pointer (check above)
        // and that its size is at least of `buf.size` bytes
        let slice = unsafe { from_raw_parts(buf.data as *const u8, buf.size) };
        let write_res = out.write_all(slice);

        // SAFETY: we are guaranteed that `buf` is a valid and initialized J* buffer (check above)
        unsafe { ffi::jsrBufferFree(&mut buf as *mut ffi::JStarBuffer) };

        match write_res {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Similar to [VM::compile] but returns the compiled bytecode as a [`Vec<u8>`].
    /// This method is convenient when the compiled bytecode needs to be stored in memory.
    ///
    /// # arguments
    ///
    /// * `path` - The path of the source code. It doesn't have to be a real filesystem path, as it
    /// is only used during error callbacks to provide useful context to the client handling the
    /// error. Nonetheless, if the source code has been indeed read from a file, it is reccomended
    /// to pass its path to this function.
    ///
    /// * `src` - The J* source code to compile
    ///
    /// # Returns
    /// Ok([`Vec<u8>`]) if the compilation succeded, Err([Error]) otherwise.
    pub fn compile_in_memory(&self, path: &str, src: &str) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.compile(path, src, &mut out)?;
        Ok(out)
    }
}

/// A 'reference' to a slot in the J* stack.
pub struct StackRef<'vm> {
    index: Index,
    vm: &'vm VM<'vm>,
}

impl<'vm> StackRef<'vm> {
    /// Get the J* value in the stack slot pointed to by this reference.
    /// If the value at the slot cannot be converted to a `T` (usually because it has the wrong J*
    /// type) returns `None`.
    pub fn get<T>(&self) -> Option<T>
    where
        T: FromJStar<'vm>,
    {
        T::from_jstar(self.vm, self.index)
    }
}

unsafe impl<'a, State> Send for VM<'a, State> {}

/// Enum that serves the purpose of tracking the ownership of a pointer to an [ffi::JStarVM].
/// Since we need the ability to construct a new rust wrapper around a `*mut JStarVM` when it is
/// needed (for example in callbakcs, where only a pointer to the vm is available), we need to
/// keep track which of the `VM` wrappers is the owner of the pointer (and thus is responsible for
/// its deallocation) and which is only a temporary wrapper (a sort of 'borrow').
/// This enum accomplishes this need, and it also mantains all of the owned state needed for the VM
/// to work.
enum VMOwnership<'a> {
    Owned(Box<Trampolines<'a>>),
    NonOwned,
}

/// Struct that owns the import and error callbacks called by J* during error handling or import
/// resolution.
/// In conjunction with [error_trampoline] and [import_trampoline] it enables the execution of these
/// functions across an ffi boundary, lifting the requirement of having to declare them as `extern "C"`
struct Trampolines<'a> {
    error_callback: Option<ErrorCallback<'a>>,
    import_callback: Option<ImportCallback<'a>>,
}

extern "C" fn error_trampoline(
    vm: *mut ffi::JStarVM,
    res: ffi::JStarResult,
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
        let err = Error::try_from(res).expect("err shouldn't be JStarResult::Success");
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
            None => ffi::JStarImportResult::default(),
            Some(module) => {
                let (code, path, reg) = match module {
                    Module::Source { src, path, reg } => (src.into(), path, reg),
                    Module::Binary { code, path, reg } => (code, path, reg),
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

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::{convert::ToJStar, CORE_MODULE, MAIN_MODULE};

    #[test]
    fn eval_string() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.eval_string("<string>", "print('Hello, World!')")
            .unwrap();
    }

    #[test]
    fn eval_bin() {
        let vm = VM::new(Conf::new());
        let code = vm
            .compile_in_memory("<string>", "print('Hello, World!')")
            .unwrap();

        let mut vm = vm.init_runtime();
        vm.eval("<string>", code).unwrap();
    }

    #[test]
    fn call() -> Result<()> {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval_string("<string>", "var add = |a, b| => a + b")?;
        vm.get_global(MAIN_MODULE, "add")?;

        3.to_jstar(&vm);
        2.to_jstar(&vm);
        vm.call(2)?;

        let n = i32::from_jstar(&vm, -1).ok_or(Error::Runtime)?;
        assert_eq!(n, 5);

        vm.pop();
        Ok(())
    }

    #[test]
    #[should_panic]
    fn call_panic() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.get_global(CORE_MODULE, "print").unwrap();
        vm.call(2).unwrap();
    }

    #[test]
    fn get_global() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval_string("<string>", "var test = 'test'").unwrap();
        vm.get_global(MAIN_MODULE, "test").unwrap();
        let s = JStarString::from_jstar(&vm, -1).unwrap();
        assert_eq!(s, "test");

        vm.pop();
    }

    #[test]
    fn get_global_fail() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval_string("<string>", "var test = 'test'").unwrap();
        let res = vm.get_global(MAIN_MODULE, "doesnotexist").unwrap_err();
        assert!(matches!(res, Error::Runtime));
    }

    #[test]
    fn set_global() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval_string("<setglb>", "var test = 'test'").unwrap();

        42.to_jstar(&vm);
        vm.set_global(MAIN_MODULE, "test");
        vm.pop();

        vm.eval_string("<setglb>", "std.assert(test == 42)")
            .unwrap();
    }

    #[test]
    fn error_callback() {
        let mut num_errors = 0;
        let conf = Conf::new().error_callback(Box::new(|_, _, _, _| {
            num_errors += 1;
        }));

        let vm = VM::new(conf);
        let mut vm = vm.init_runtime();

        let err = vm.eval_string("<string>", "raise Exception()").unwrap_err();
        assert!(matches!(err, Error::Runtime));

        let err = vm.eval_string("<string>", "for end").unwrap_err();
        assert!(matches!(err, Error::Syntax));

        let err = vm
            .eval_string("<string>", "begin var a; var a; end")
            .unwrap_err();
        assert!(matches!(err, Error::Compile));

        vm.eval_string("<string>", "var bar = 1 + 2").unwrap();

        drop(vm);

        assert_eq!(num_errors, 3);
    }

    #[test]
    fn import_source() {
        let conf = Conf::new().import_callback(Box::new(|_, module_name| {
            if module_name == "test" {
                Some(Module::source(
                    "var flag = 1".to_owned(),
                    "<test>".to_owned(),
                ))
            } else {
                None
            }
        }));

        let vm = VM::new(conf);
        let mut vm = vm.init_runtime();

        vm.eval_string(
            "<string>",
            "import test
            std.assert(test.flag == 1)",
        )
        .unwrap();

        let err = vm
            .eval_string("<string>", "import does_not_exist")
            .unwrap_err();
        assert!(matches!(err, Error::Runtime));
    }

    #[test]
    fn import_binary() {
        let mut err_called = false;

        let conf = Conf::new()
            .error_callback(Box::new(|err, path, line, msg| {
                assert!(matches!(err, Error::Runtime));
                assert_eq!(path, "<string>");
                assert!(line.is_none());
                assert_eq!(msg, "Traceback (most recent call last):\n    [line 1] module __main__ in <main>\nImportException: Cannot load module `does_not_exist`.");
                err_called = true;
            }))
            .import_callback(Box::new(|vm, module_name| {
            if module_name == "test" {
                Some(Module::binary(
                    vm.compile_in_memory("<test>", "var flag = 1").unwrap(),
                    "<test>".to_owned(),
                ))
            } else {
                None
            }
        }));

        let vm = VM::new(conf);
        let mut vm = vm.init_runtime();

        vm.eval_string(
            "<string>",
            "import test
            std.assert(test.flag == 1)",
        )
        .unwrap();

        let err = vm
            .eval_string("<string>", "import does_not_exist")
            .unwrap_err();
        assert!(matches!(err, Error::Runtime));

        drop(vm);

        assert!(err_called);
    }

    #[test]
    fn push_get_number() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        vm.push_number(46.0);
        let n = vm.get_number(-1).unwrap();
        assert_eq!(46.0, n);
    }

    #[test]
    #[should_panic]
    fn get_number_panic() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        let _ = vm.get_number(-1);
    }

    #[test]
    fn get_number_none() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        vm.push_string("notanumber");
        let n = vm.get_number(-1);
        assert!(n.is_none());
    }

    #[test]
    fn push_get_string() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        vm.push_string("test");
        let s = vm.get_string(-1).unwrap();
        assert_eq!(s, "test");
    }

    #[test]
    #[should_panic]
    fn get_string_panic() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        let _ = vm.get_string(-1).unwrap();
    }

    #[test]
    fn get_string_none() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        vm.push_number(2.0);
        let s = vm.get_string(-1);
        assert!(s.is_none());
    }

    #[test]
    fn pop() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.push_number(2.0);
        vm.push_string("test");
        vm.pop();
        let s = vm.get_number(-1).unwrap();
        assert_eq!(s, 2.0);
    }

    #[test]
    #[should_panic]
    fn pop_panic() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.pop();
    }

    #[test]
    fn pop_n() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.push_number(2.0);
        vm.push_number(3.0);
        vm.push_number(4.0);
        vm.push_number(5.0);
        vm.pop_n(3);
        let n = vm.get_number(-1).unwrap();
        assert_eq!(n, 2.0);
    }

    #[test]
    #[should_panic]
    fn pop_n_panic() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();
        vm.push_number(2.0);
        vm.push_number(3.0);
        vm.push_number(4.0);
        vm.push_number(5.0);
        vm.pop_n(5);
    }

    #[test]
    fn validate_slot_success() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        vm.push_number(5.0);
        assert!(vm.validate_slot(-1));
    }

    #[test]
    fn validate_slot_fail() {
        let vm = VM::new(Conf::new());
        let vm = vm.init_runtime();
        assert!(!vm.validate_slot(-1));
    }
}
