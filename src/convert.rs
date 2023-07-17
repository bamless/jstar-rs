use crate::vm::{VM, Index};

pub trait ToJStar {
    fn to_jstar(&self, vm: &mut VM);
}

pub trait FromJStar: Sized {
    fn from_jstar(vm: &VM, slot: Index) -> Option<Self>;
}

impl ToJStar for f64 {
    fn to_jstar(&self, vm: &mut VM) {
        vm.push_number(*self);
    }
}

impl FromJStar for f64 {
    fn from_jstar(vm: &VM, slot: Index) -> Option<Self> {
        vm.get_number(slot)
    }
}
