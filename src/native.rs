// TODO: documentation
#[macro_export]
macro_rules! native {
    ($v:vis fn $name:ident($arg:ident) $b:block) => {
        #[allow(non_snake_case)]
        $v extern "C" fn $name(vm: *mut $crate::ffi::JStarVM) -> std::os::raw::c_int {
            let mut vm = unsafe { $crate::vm::VM::from_ptr(vm) };
            let $arg = &mut vm;
            let res: $crate::error::Result<()> = $b;
            match res {
                Err(_) => 0,
                Ok(()) => 1,
            }
        }
    };
}

// TODO: test this
