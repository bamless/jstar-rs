use crate::{vm::{Index, VM}, string::String};

/// Trait used to push a value onto the J* stack.
/// Types that implement this trait usually have a corresponding `push_...` method in the [VM].
pub trait ToJStar {
    /// Pushes the value onto the J* stack
    fn to_jstar(&self, vm: &VM);
}

impl<T: Into<f64> + Copy> ToJStar for T {
    fn to_jstar(&self, vm: &VM) {
        vm.push_number((*self).into());
    }
}

impl ToJStar for str {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl ToJStar for [u8] {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl<'vm> ToJStar for String<'vm> {
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self.as_ref());
    }
}

/// Trait used to get a value from the J* stack.
/// Types that implement this trait usually have corresponding `get_...` and `is_...` methods in the [VM]
pub trait FromJStar<'vm>: Sized {
    /// Get the value from the J* stack at `slot`.
    /// If the value at `slot` is not of type `Self` this method returns `None`.
    fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self>;
}

impl<'vm, T: TryFrom<f64>> FromJStar<'vm> for T {
    fn from_jstar(vm: &VM, slot: Index) -> Option<Self> {
        match vm.get_number(slot) {
            Some(n) => Self::try_from(n).ok(),
            None => None,
        }
    }
}

impl<'vm> FromJStar<'vm> for String<'vm> {
    fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self> {
        vm.get_string(slot)
    }
}
