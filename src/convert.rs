use crate::vm::{Index, VM};

pub trait ToJStar {
    fn to_jstar(&self, vm: &mut VM);
}

pub trait FromJStar: Sized {
    fn from_jstar(vm: &VM, slot: Index) -> Option<Self>;
}

impl<T: Into<f64> + Copy> ToJStar for T {
    fn to_jstar(&self, vm: &mut VM) {
        vm.push_number((*self).into());
    }
}

impl<T: TryFrom<f64>> FromJStar for T {
    fn from_jstar(vm: &VM, slot: Index) -> Option<Self> {
        match vm.get_number(slot) {
            Some(n) => Self::try_from(n).ok(),
            None => None,
        }
    }
}
