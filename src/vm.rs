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
///
/// An uninitialized vm doesn't have a language runtime yet, so it can only perform operations that
/// don't require one, such as compiling J* code or allocating vm-managed buffers.
/// To obtain a fully initialized vm that can execute code call the [VM::init_runtime] method.
/// Keep in mind that initializing the runtime *will* execute J* code and allocate memory and as
/// such it's a (relatively) slow process. Only call the initialization when needed and outside
/// performance critical sections.
pub struct Uninit;

/// Marker struct that represents a fully initialized J* vm.
///
/// Capable of executing J* code, as well as performing any operations an [Uninit] vm can.
pub struct Init;

/// The J* virtual machine.
///
/// This is the main struct used to execute J* code and interact with the J* runtime.
///
/// # Examples
///
/// The following examples show some pretty basic usage of the VM struct for calling J* code from
/// Rust and vice versa. For more detailed information on the J* language as a whole and its
/// stack-based communication protocol for embedding, refer to the [J* language
/// reference](https://bamless.github.io/jstar).
///
/// ## Calling J* code from Rust
///
/// To instantiate a new J* vm and evaluate some code:
/// ```rust
/// use jstar::{conf::Conf, vm::VM};
///
/// // `init_runtime` is needed for the VM to be capable of executing J* code.
/// let vm = VM::new(Conf::new()).init_runtime();
/// vm.eval("<eval>", "print('Hello from Rust!')").unwrap();
/// ```
///
/// Using the [VM::eval] (or [VM::eval_in_module]) method is the simplest way to start evaluating
/// J* code from Rust, but it is pretty limiting in certain situations, as it doesn't allow for
/// retrieving values from the J* code or passing arguments to it.
///
/// Usually, you will use the `eval` methods to execute some code that declares functions, classes
/// or variables (or that does some other declaative things, like importing a module), and then use
/// the other provided methods to interact directly with J* `Value`s using the stack-based
/// communication protocol of the J* VM.  
/// For example, suppose we want to define an `match` function that uses the built-in `re` module
/// of J* to find if a given rust string matches a given pattern:
/// ```rust
/// # use jstar::{
/// #     conf::Conf,
/// #     vm::VM,
/// #     convert::ToJStar,
/// #     string::String,
/// #     convert::FromJStar,
/// #     MAIN_MODULE
/// # };
/// let mut vm = VM::new(Conf::new()).init_runtime();
///
/// vm.eval("<eval>", "
/// import re
///
/// fun matches(string, pattern)
///     return re.match(string, pattern)
/// end")
/// .expect("`eval` to succed");
///
/// // `matches("Hello, World!", "[wW]orld!?")`
/// vm.get_global(MAIN_MODULE, "matches").expect("`get_global` to succeed");
/// "Hello, World!".to_jstar(&vm);
/// "[wW]orld!?".to_jstar(&vm);
/// vm.call(2).expect("`call` to succeed");
///
/// let re_match = String::from_jstar(&vm, -1).expect("`get_string` to succeed");
/// assert_eq!(re_match, "World!");
///
/// // We are done with the result, pop it from the stack
/// vm.pop();
/// ```
///
/// This is just a subset of the things you can do with the J* VM from the embedded side. For a
/// more complete overview of the J* language and its capabilities, refer to the [J* language
/// reference](https://bamless.github.io/jstar), and the documentation of `impl VM` methods.
///
/// ## Calling Rust code from J*
///
/// Other than calling J* code from Rust, you can also call Rust code from J* using the native
/// registration API. We will focus on the simplest case, i.e. registering a function directly
/// using the `native!` macro along with the `register_native` method. To see how to dynamically
/// load a shared library and register functions from it, refer to the [J* language reference](https://bamless.github.io/jstar)
/// and the [`crate::import`] module.
/// ```rust
/// # use jstar::{
/// #     conf::Conf,
/// #     vm::VM,
/// #     convert::ToJStar,
/// #     convert::FromJStar,
/// #     MAIN_MODULE,
/// #     native,
/// # };
/// let vm = VM::new(Conf::new()).init_runtime();
///
/// native!(fn rustAdd(vm) {
///     // First argument
///     let a = i32::from_jstar_checked(vm, 1, "a")?;
///     // Second argument
///     let b = i32::from_jstar_checked(vm, 2, "b")?;
///
///     (a + b).to_jstar(vm);
///
///     Ok(())
/// });
///
/// vm.register_native(MAIN_MODULE, "rustAdd", rustAdd, 2).expect("`register_native` to succeed");
/// vm.eval("<eval>", "std.assert(rustAdd(2, 3) == 5)").expect("`eval` to succeed");
/// ```
///
/// # Configuration
///
/// Refer to [`Conf`] for information on how to configure the VM.
///
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

/// Methods available only when the [`VM`] is in an [Uninit]ialized state.
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
    ///
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

/// Methods available only when the [`VM`] is in an [Init]ialized state, i.e. [`VM::init_runtime`]
/// has been called.
impl<'a> VM<'a, Init> {
    /// Construct a new [VM] wrapper starting from a raw [ffi::JStarVM] pointer.
    ///
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

    /// Evaluate J* source or compiled code in the context of the `__main__` module.
    ///
    /// # Arguments
    ///
    /// * `path` - A string representing the code path. It doesn't have to be a real filesystem
    /// path, as it is only used during error callbacks to provide useful context to the client
    /// handling the error. Nonetheless, if the source code has been indeed read from a file, it
    /// is reccomended to pass its path to this function.
    ///
    /// * `code` - The J* source or compiled code to evaluate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the evaluation succeded, `Err(`[`Error::Runtime`]`)` otherwise.
    pub fn eval(&self, path: &str, code: impl AsRef<[u8]>) -> Result<()> {
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

    /// Similar to [VM::eval] but it evaluates the code in the context of `module` instead of the
    /// main module.
    ///
    /// # Arguments
    ///
    /// * `path` - A string representing the code path. It doesn't have to be a real filesystem
    /// path, as it is only used during error callbacks to provide useful context to the client
    /// handling the error. Nonetheless, if the source code has been indeed read from a file, it
    /// is reccomended to pass its path to this function.
    ///
    /// * `module` - The name of the module in which to evaluate the code. Can be any valid J*
    ///    module name or [CORE_MODULE](../constant.CORE_MODULE.html)/[MAIN_MODULE](../constant.MAIN_MODULE.html)
    ///
    /// * `code` - The J* source or compiled code to evaluate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the evaluation succeded, `Err(`[`Error::Runtime`]`)` otherwise.
    pub fn eval_in_module(&self, path: &str, module: &str, code: impl AsRef<[u8]>) -> Result<()> {
        let path = CString::new(path).expect("Couldn't create CString");
        let module = CString::new(module).expect("Couldn't create CString");
        let res = unsafe {
            ffi::jsrEvalModule(
                self.vm,
                path.as_ptr(),
                module.as_ptr(),
                code.as_ref().as_ptr() as *const c_void,
                code.as_ref().len(),
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
    /// `Ok(())` if the call succeded leaving the result on top of the stack, `Err(`[`Error::Runtime`]`)`
    /// if the the call failed leaving an Exception on top of the stack. In both cases, the args
    /// and the callee are popped from the stack.
    ///
    /// # Errors
    ///
    /// This function panics if the stack underflows or overflows the stack (for the current stack
    /// frame).
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
    ///
    /// # Errors
    ///
    /// This method panics if we try to pop more items than the stack holds (for the current stack
    /// frame).
    pub fn pop(&mut self) {
        assert!(self.validate_slot(-1), "VM stack underflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPop(self.vm) };
    }

    /// Pops `n` elements from the VM stack
    ///
    /// # Errors
    ///
    /// This method panics if we try to pop more items than the stack holds (for the current stack
    /// frame).
    pub fn pop_n(&mut self, n: i32) {
        assert!(n > 0, "`n` must be greater than 0");
        assert!(self.validate_slot(-n), "VM stack underflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPopN(self.vm, n) };
    }

    /// Push a `Number` onto the VM stack.
    ///
    /// # Errors
    ///
    /// This method panics if there isn't enough stack space for one element. Use
    /// [VM::ensure_stack] if you are not sure the stack has enough space.
    pub fn push_number(&self, number: f64) {
        assert!(self.validate_stack(), "VM stack overflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPushNumber(self.vm, number) };
    }

    /// Returns wether or not the value at `slot` is a `Number`.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
    pub fn is_number(&self, slot: Index) -> bool {
        assert!(self.validate_slot(slot), "VM stack overflow");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrIsNumber(self.vm, slot) }
    }

    /// Gets a J* `Number` from the stack.
    ///
    /// # Returns
    ///
    /// `None` if the value at `slot` is not a `Number`, the `Number` as an [f64] otherwise.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
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
    ///
    /// # Returns
    ///
    /// `Ok(`[`f64`]`)` if the value at `slot` is a `Number`, `Err(`[`Error::Runtime`]`)` otherwise,
    /// leaving a `TypeException` on the stack.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
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
    ///
    /// Since a J* string can contain arbitrary bytes, this method accepts anything that can be
    /// treated as a byte slice. The data will be copied into a J* `String` before being pushed onto
    /// the [VM] stack.
    ///
    /// # Errors
    ///
    /// This method panics if there isn't enough stack space for one element. Use [VM::ensure_stack]
    /// if you are not sure the stack has enough space.
    pub fn push_string(&self, str: impl AsRef<[u8]>) {
        let str = str.as_ref();
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrPushStringSz(self.vm, str.as_ptr() as *const c_char, str.len()) }
    }

    /// Returns wether or not the value at `slot` is a J* `String`.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
    pub fn is_string(&self, slot: Index) -> bool {
        assert!(self.validate_slot(slot), "`slot` out of bounds");
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrIsString(self.vm, slot) }
    }

    /// Gets a J* `String` from the stack.
    ///
    /// # Returns
    ///
    /// `Some(`[JStarString]`)` if the value at `slot` is a `String`, `None` otherwise.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
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
    ///
    /// # Returns
    ///
    /// `Ok(`[`JStarString`]`)` if the value at `slot` is a `Number`.  
    /// `Err(`[`Error::Runtime`]`)` otherwise, leaving a `TypeException` on the stack.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows or overflows the stack (for the current stack
    /// frame).
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
    /// `Ok(())` in case of success leaving the value on top of the stack.  
    /// `Err(`[`Error::Runtime`]`)` in case of failure leaving an exception on top of the stack.
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
    ///
    /// # Arguments
    ///
    /// * `module_name` - The name of the module in which to set the global. Could be any valid J*
    ///    module name or [CORE_MODULE](../constant.CORE_MODULE.html)/[MAIN_MODULE](../constant.MAIN_MODULE.html)
    ///    for the two built-in modules.
    /// * `name` - The name of the global variable to set.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, leaving the value on top of the stack.
    /// `Err(`[`Error::Runtime`]`)` in case of failure, leaving an exception on top of the stack.
    pub fn set_global(&self, module_name: &str, name: &str) -> Result<()> {
        // TODO: check that `module_name` exists. New J* apis should be added for this.
        assert!(self.validate_slot(-1));
        let module_name = CString::new(module_name).expect("`module` to be a valid CString");
        let name = CString::new(name).expect("`name` to be a valid CString");
        let res = unsafe { ffi::jsrSetGlobal(self.vm, module_name.as_ptr(), name.as_ptr()) };
        if !res {
            Err(Error::Runtime)
        } else {
            Ok(())
        }
    }

    /// Pushes a naive function onto the stack.
    ///
    /// See [crate::native!] for utility functions and macros to create natives.
    ///
    /// # Arguments
    ///
    /// * `module` - The name of the module the function belongs to
    /// * `name` - The name of the function
    /// * `func` - The native function to push
    /// * `argc` - The number of arguments the function takes
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, leaving the native function on top of the stack.
    /// `Err(`[`Error::Runtime`]`)` in case of failure, leaving an exception on top of the stack.
    pub fn push_native(
        &self,
        module: &str,
        name: &str,
        func: ffi::JStarNative,
        argc: u8,
    ) -> Result<()> {
        let module = CString::new(module).expect("`module` to be a valid CString");
        let name = CString::new(name).expect("`name` to be a valid CString");
        let res =
            unsafe { ffi::jsrPushNative(self.vm, module.as_ptr(), name.as_ptr(), func, argc) };
        if !res {
            Err(Error::Runtime)
        } else {
            Ok(())
        }
    }

    /// Registers a native function in the global scope of module `module`.
    ///
    /// This is a convenience method that is mostly equivalent to doing:
    /// ```rust
    /// # use jstar::{vm::VM, conf::Conf, native};
    /// # let mut vm = VM::new(Conf::new()).init_runtime();
    /// # let module = "module";
    /// # let name = "func_name";
    /// # let argc = 0;
    /// # native!(fn func(vm) { Ok(()) });
    /// vm.push_native(module, name, func, argc);
    /// vm.set_global(module, name);
    /// vm.pop();
    /// ```
    ///
    /// # Arguments
    ///
    /// * `module` - The name of the module the function belongs to
    /// * `name` - The name the function will be bound to
    /// * `func` - The native function to register
    /// * `argc` - The number of arguments the function takes
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    /// `Err(`[`Error::Runtime`]`)` in case of failure, leaving an exception on top of the stack.
    pub fn register_native(
        &self,
        module: &str,
        name: &str,
        func: ffi::JStarNative,
        argc: u8,
    ) -> Result<()> {
        self.push_native(module, name, func, argc)?;
        self.set_global(module, name)?;
        // SAFETY: `self.vm` is a valid J* vm pointer and we are guaranteed that the stack will
        // not underflow (we just pushed ane element)
        unsafe { ffi::jsrPop(self.vm) };
        Ok(())
    }

    /// Returns a [`StackRef`] pointing to the topmost stack slot.
    pub fn get_top(&self) -> StackRef {
        StackRef {
            // SAFETY: `self.vm` is a valid J* vm pointer
            index: unsafe { ffi::jsrTop(self.vm) },
            vm: self,
        }
    }

    /// Returns a [`StackRef`] pointing to the stack slot at `slot`.
    /// `slot` is treated as an offset from the top of the stack and must be positive.
    ///
    /// # Errors
    ///
    /// This method panics if the slot underflows the stack (for the current stack frame).
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
    ///
    /// See [`native::MIN_NATIVE_STACK_SZ`](../native/constant.MIN_NATIVE_STACK_SZ.html) for the
    /// minimum guaranteed stack size when calling a J* native function.
    pub fn ensure_stack(&self, needed: usize) {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrEnsureStack(self.vm, needed) };
    }

    /// Returns `true` if the provided slot is valid, i.e. it doesn't overflow or underflow the
    /// stack, false otherwise
    pub fn validate_slot(&self, slot: Index) -> bool {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrValidateSlot(self.vm, slot) }
    }

    /// Returns `true` if the stack has space for one element, i.e. pushing one element will not
    /// overflow the stack
    pub fn validate_stack(&self) -> bool {
        // SAFETY: `self.vm` is a valid J* vm pointer
        unsafe { ffi::jsrValidateStack(self.vm) }
    }
}

/// Methods available to both [Init] and [Uninit] VMs.
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
    /// * `out` - A [`Write`] implementor to write the compiled bytecode to
    ///
    /// # Returns
    ///
    /// `Ok(())` if the compilation succeded, `Err(`[`Error`]`)` otherwise.
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
    /// `Ok(`[`Vec<u8>`]`)` if the compilation succeded, `Err(`[`Error`]`)` otherwise.
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
    use crate::{convert::ToJStar, native, CORE_MODULE, MAIN_MODULE};

    #[test]
    fn eval() {
        let vm = VM::new(Conf::new()).init_runtime();
        vm.eval("<string>", "print('Hello, World!')").unwrap();
    }

    #[test]
    fn eval_bin() {
        let vm = VM::new(Conf::new());
        let code = vm
            .compile_in_memory("<string>", "print('Hello, World!')")
            .unwrap();

        let vm = vm.init_runtime();
        vm.eval("<string>", code).unwrap();
    }

    #[test]
    fn eval_in_module() {
        let mut vm = VM::new(Conf::new()).init_runtime();
        vm.eval_in_module("<string>", "test", "var x = 42").unwrap();

        vm.get_global("test", "x").unwrap();
        assert!(i32::from_jstar(&vm, -1).unwrap() == 42);
        vm.pop();

        let res = vm.get_global(MAIN_MODULE, "x");
        assert!(matches!(res, Err(Error::Runtime)));
        vm.pop();
    }

    #[test]
    fn eval_in_module_bin() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        let code = vm.compile_in_memory("<test>", "var x = 42").unwrap();
        vm.eval_in_module("<string>", "test", code).unwrap();

        vm.get_global("test", "x").unwrap();
        assert!(i32::from_jstar(&vm, -1).unwrap() == 42);
        vm.pop();

        let res = vm.get_global(MAIN_MODULE, "x");
        assert!(matches!(res, Err(Error::Runtime)));
        vm.pop();
    }

    #[test]
    fn call() -> Result<()> {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval("<string>", "var add = |a, b| => a + b")?;
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
        let mut vm = VM::new(Conf::new()).init_runtime();
        vm.get_global(CORE_MODULE, "print").unwrap();
        vm.call(2).unwrap();
    }

    #[test]
    fn get_global() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        vm.eval("<string>", "var test = 'test'").unwrap();
        vm.get_global(MAIN_MODULE, "test").unwrap();
        let s = JStarString::from_jstar(&vm, -1).unwrap();
        assert_eq!(s, "test");

        vm.pop();
    }

    #[test]
    fn get_global_fail() {
        let vm = VM::new(Conf::new()).init_runtime();
        vm.eval("<string>", "var test = 'test'").unwrap();
        let res = vm.get_global(MAIN_MODULE, "doesnotexist").unwrap_err();
        assert!(matches!(res, Error::Runtime));
    }

    #[test]
    fn get_global_fail_module() {
        let vm = VM::new(Conf::new()).init_runtime();
        let res = vm.get_global("does_not_exist", "doesnotexist").unwrap_err();
        assert!(matches!(res, Error::Runtime));
    }

    #[test]
    fn set_global() {
        let vm = VM::new(Conf::new());
        let mut vm = vm.init_runtime();

        vm.eval("<setglb>", "var test = 'test'").unwrap();

        42.to_jstar(&vm);
        vm.set_global(MAIN_MODULE, "test").unwrap();
        vm.pop();

        vm.eval("<setglb>", "std.assert(test == 42)").unwrap();
    }

    #[test]
    fn set_global_fail() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        42.to_jstar(&vm);
        let res = vm.set_global("does_not_exist", "test");
        vm.pop();

        assert!(matches!(res, Err(Error::Runtime)));
    }

    #[test]
    fn push_native() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            let n = vm.get_number(1).unwrap();
            n.to_jstar(vm);
            Ok(())
        });

        vm.push_native(MAIN_MODULE, "id", id, 1).unwrap();
        vm.set_global(MAIN_MODULE, "id").unwrap();
        vm.pop();

        vm.eval("<string>", "std.assert(id(42) == 42)").unwrap();
    }

    #[test]
    fn push_native_fail() {
        let vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            let n = vm.get_number(1).unwrap();
            n.to_jstar(vm);
            Ok(())
        });

        let res = vm.push_native("does_not_exist", "id", id, 1);
        assert!(matches!(res, Err(Error::Runtime)));
    }

    #[test]
    fn register_native() {
        let vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            let n = vm.get_number(1).unwrap();
            n.to_jstar(vm);
            Ok(())
        });

        vm.register_native(MAIN_MODULE, "id", id, 1).unwrap();

        vm.eval("<string>", "std.assert(id(42) == 42)").unwrap();
    }

    #[test]
    fn register_native_fail() {
        let vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            let n = vm.get_number(1).unwrap();
            n.to_jstar(vm);
            Ok(())
        });

        let res = vm.register_native("does_not_exist", "id", id, 1);
        assert!(matches!(res, Err(Error::Runtime)));
    }

    #[test]
    fn native_call_fail() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            let n = i32::from_jstar_checked(vm, 1, "n")?;
            n.to_jstar(vm);
            Ok(())
        });

        vm.register_native(MAIN_MODULE, "id", id, 1).unwrap();

        vm.get_global(MAIN_MODULE, "id").unwrap();
        "not a number".to_jstar(&vm);

        let res = vm.call(1);
        assert!(matches!(res, Err(Error::Runtime)));

        vm.pop();

        vm.get_global(MAIN_MODULE, "id").unwrap();
        42.to_jstar(&vm);
        34.to_jstar(&vm);

        let res = vm.call(2);
        assert!(matches!(res, Err(Error::Runtime)));
    }

    #[test]
    #[should_panic]
    fn native_should_panic() {
        let mut vm = VM::new(Conf::new()).init_runtime();

        native!(fn id(vm) {
            // Try to pop past stack frame boundary
            vm.pop_n(2);
            Ok(())
        });

        vm.register_native(MAIN_MODULE, "id", id, 0).unwrap();
        vm.get_global(MAIN_MODULE, "id").unwrap();
        vm.call(0).unwrap();
    }

    #[test]
    fn error_callback() {
        let mut num_errors = 0;
        let conf = Conf::new().error_callback(Box::new(|_, _, _, _| {
            num_errors += 1;
        }));

        let vm = VM::new(conf).init_runtime();

        let err = vm.eval("<string>", "raise Exception()").unwrap_err();
        assert!(matches!(err, Error::Runtime));

        let err = vm.eval("<string>", "for end").unwrap_err();
        assert!(matches!(err, Error::Syntax));

        let err = vm.eval("<string>", "begin var a; var a; end").unwrap_err();
        assert!(matches!(err, Error::Compile));

        vm.eval("<string>", "var bar = 1 + 2").unwrap();

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

        let vm = VM::new(conf).init_runtime();

        vm.eval(
            "<string>",
            "import test
            std.assert(test.flag == 1)",
        )
        .unwrap();

        let err = vm.eval("<string>", "import does_not_exist").unwrap_err();
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
            }
        ));

        let vm = VM::new(conf).init_runtime();

        vm.eval(
            "<string>",
            "import test
            std.assert(test.flag == 1)",
        )
        .unwrap();

        let err = vm.eval("<string>", "import does_not_exist").unwrap_err();
        assert!(matches!(err, Error::Runtime));

        drop(vm);

        assert!(err_called);
    }

    #[test]
    #[should_panic]
    fn import_stack_underflow_panic() {
        // This should panic, as we're popping past the stack frame boundary
        // This should mantain the invariant that `string_ref` must remain valid and not dangle
        // by being popped off the stack (as will happen if the code below is permitted)
        let conf = Conf::new().import_callback(Box::new(|vm, _| {
            vm.pop();
            vm.pop();
            None
        }));

        let vm = VM::new(conf).init_runtime();
        "string".to_jstar(&vm);

        let string_ref = JStarString::from_jstar(&vm, -1).unwrap();
        let _ = vm.eval("<string>", "import test");

        assert_eq!(string_ref, "string");
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
