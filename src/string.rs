use std::hash::Hash;
use std::{ffi::c_char, marker::PhantomData};

use crate::vm::VM;

#[derive(Eq)]
pub struct String<'vm> {
    data: *const c_char,
    len: usize,
    phantom: PhantomData<&'vm VM<'vm>>
}

impl<'vm> String<'vm> {
    pub(crate) fn new(data: *const c_char, len: usize, _vm: &VM) -> Self {
        String {
            data,
            len,
            phantom: PhantomData,
        }
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: We know the vm is still valid (`phantom` lifetime). Also, as we have an
        // exclusive reference to the vm, we know the J* string couldn't have been possibly popped
        // from the stack, so we are guaranteed that the `data` pointer is still valid
        unsafe { std::slice::from_raw_parts(self.data as *const u8, self.len) }
    }
}

impl<'vm> AsRef<[u8]> for String<'vm> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'vm, T: AsRef<[u8]>> PartialEq<T> for String<'vm> {
    fn eq(&self, other: &T) -> bool {
        self.as_bytes() == other.as_ref()
    }
}

impl<'vm> Hash for String<'vm> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}
