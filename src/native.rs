/// The minimum size of the native stack (in slots) when calling a native function.
///
/// If you need to use more slots than this, you should ensure enough stack space is available by
/// calling [`crate::vm::VM::ensure_stack`] before calling the function.
pub const MIN_NATIVE_STACK_SZ: usize = crate::ffi::JSTAR_MIN_NATIVE_STACK_SZ;

/// Macro to define a native function.
///
/// The function takes in a `&mut `[`crate::vm::VM`] as its only argument and must return a
/// [`Result`] where the [Ok] variant is `()` and the [Err] variant is [`crate::error::Error`].
///
/// # Example
///
/// ```
/// # use jstar::{native, vm::VM, conf::Conf, MAIN_MODULE};
/// # let mut vm = VM::new(Conf::new()).init_runtime();
/// // The `vm` argument is a mutable reference to the J* VM (&mut VM).
/// native!(fn nativeFn(vm) {
///     // Your code here
///     // ...
///     // Return a `Result<(), crate::error::Error>`
///     Ok(())
/// });
/// # vm.register_native(MAIN_MODULE, "nativeFn", nativeFn, 0);
/// # vm.get_global(MAIN_MODULE, "nativeFn").unwrap();
/// # vm.call(0).unwrap();
/// ```
#[macro_export]
macro_rules! native {
    ($v:vis fn $name:ident($arg:ident) $b:block) => {
        #[allow(non_snake_case)]
        $v extern "C" fn $name(vm: *mut $crate::ffi::JStarVM) -> bool {
            let mut vm = unsafe { $crate::vm::VM::from_ptr(vm) };
            let $arg = &mut vm;
            let func = |$arg: &mut $crate::vm::VM| -> $crate::error::Result<()> { $b };
            let res = func($arg);
            match res {
                Err(_) => false,
                Ok(()) => true,
            }
        }
    };
}
