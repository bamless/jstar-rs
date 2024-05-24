use crate::vm::VM;

use std::{ffi::c_char, hash::Hash, marker::PhantomData};

/// `String` represents a J* string.
/// In J* `String`s are basically `&[u8]` as they can store arbitrary data and their encoding is not
/// assumed.
#[derive(Debug, Eq)]
pub struct String<'vm> {
    data: *const c_char,
    len: usize,
    phantom: PhantomData<&'vm VM<'vm>>,
}

impl<'vm> String<'vm> {
    /// Construct a new [String] starting from a pointer and a length to a J* `String`.
    pub(crate) fn new(data: *const c_char, len: usize) -> Self {
        String {
            data,
            len,
            phantom: PhantomData,
        }
    }

    /// Returns this String as a [&str].
    /// As `String`s in J* can store arbitrary data and their encoding is not assumed, this method
    /// may return an utf8 encoding error
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    /// Returns this String as a &[[u8]]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: We know the vm is still valid (`self.phantom` lifetime). Also, as we have an
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
