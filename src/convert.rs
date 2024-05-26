use crate::{
    error::Result, string::String, vm::{Index, VM}
};

macro_rules! to_jstar_number_impl {
    ($($t:ty),*) => {
        $(impl ToJStar for $t {
            fn to_jstar(&self, vm: &VM) {
                vm.push_number(*self as f64);
            }
        }
        impl ToJStar for &$t {
            fn to_jstar(&self, vm: &VM) {
                (*self).to_jstar(vm);
            }
        })*
    };
}

macro_rules! from_jstar_number_impl {
    ($($t:ty),*) => {
        $(impl<'vm> FromJStar<'vm> for $t {
            fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self> {
                vm.get_number(slot).map(|n| n as $t)
            }

            fn from_jstar_checked(vm: &'vm VM, slot: Index, name: &str) -> $crate::error::Result<Self> {
                vm.check_number(slot, name).map(|n| n as $t)
            }
        })*
    };
}

/// Trait used to push a value onto the J* stack.
/// Types that implement this trait usually have a corresponding `push_...` method in the [VM].
pub trait ToJStar {
    /// Pushes the value onto the J* stack
    fn to_jstar(&self, vm: &VM);
}

to_jstar_number_impl!(f64, f32, u64, u32, u16, u8, i64, i32, i16, i8);

impl ToJStar for &str {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl ToJStar for &[u8] {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl<'vm> ToJStar for String<'vm> {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self.as_ref());
    }
}

impl<'vm> ToJStar for &String<'vm> {
    fn to_jstar(&self, vm: &VM) {
        (*self).to_jstar(vm);
    }
}

/// Trait used to get a value from the J* stack.
/// Types that implement this trait usually have corresponding `get_...`, `is_...` and `check` methods in the [VM]
pub trait FromJStar<'vm>: Sized {
    /// Get the value from the J* stack at `slot`.
    /// If the value at `slot` is not of type `Self` this method returns `None`.
    fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self>;

    /// Get the value from the J* stack at `slot`.
    /// If the value at `slot` is not of type `Self` this method returns an error and leaves a
    /// `TypeException` on top of the stack.
    fn from_jstar_checked(vm: &'vm VM, slot: Index, name: &str) -> Result<Self>;
}

from_jstar_number_impl!(f64, f32, u64, u32, u16, u8, i64, i32, i16, i8);

impl<'vm> FromJStar<'vm> for String<'vm> {
    fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self> {
        vm.get_string(slot)
    }

    fn from_jstar_checked(vm: &'vm VM, slot: Index, name: &str) -> Result<Self> {
        vm.check_string(slot, name)
    }
}
