// TODO: documentation
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

// TODO: test this
