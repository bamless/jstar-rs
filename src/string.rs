use crate::{convert::{FromJStar, ToJStar}, vm::{Index, VM}};

use std::{ffi::c_char, hash::Hash, marker::PhantomData};

/// [String] represents a J* `String`.
///
/// A J* `String` is more akin to a byte slice than a Rust [std::string::String], as it can store
/// arbitrary data and its encoding is not assumed. For this reason, the [String] type implements
/// [`AsRef<[u8]>`] instead of [`AsRef<str>`]. To convert a J* string into a &[str], see the
/// [String::as_str] method.
///
/// [String] acts as a thin wrapper around a J* string stored on the [VM] stack. For this reason,
/// it is bound to the lifetime of the [VM] it was created from. Furthermore, [String] holds a
/// shared reference to the [VM], so it is not possible to mutate the [VM] stack while a [String] is
/// being held. This ensures that [String] always points to the right stack slot and that the
/// underlying memory cannot be reclaimed by the J* GC while it is still being used. For example:
/// ```compile_fail
/// # use jstar::{conf::Conf, string::String, vm::VM, convert::{ToJStar, FromJStar}};
/// # let mut vm = VM::new(Conf::new()).init_runtime();
/// "string from rust".to_jstar(&vm);
///
/// // This J* string 'points' to the topmost stack slot
/// let jstar_string = String::from_jstar(&vm, -1).unwrap();
///
/// // This would pop the J* string from the stack, making `jstar_string` dangling and making the
/// // string reclaimable by an eventual GC cycle.
/// // For this, it will be prevented by the borrow checker.
/// vm.pop();
///
/// println!("{}", jstar_string.as_str().expect("To be valid utf8"));
/// ```
///
/// To allieviate this requirement, you can always clone the string to an owned [std::string::String]
/// or a [`Vec<u8>`], either by converting a reference to an owned type or by using the provided [From]
/// and [TryFrom] implementations:
/// ```rust
/// # use jstar::{conf::Conf, string::String, vm::VM, convert::{ToJStar, FromJStar}};
/// # let mut vm = VM::new(Conf::new()).init_runtime();
/// "string from rust".to_jstar(&vm);
///
/// // This J* string 'points' to the topmost stack slot
/// let jstar_string = String::from_jstar(&vm, -1).unwrap();
/// let owned: std::string::String = jstar_string.try_into().expect("To be valid utf8");
///
/// // This'll be allowed now, as we're not using the J* string anymore after this point, only the
/// // owned copy.
/// vm.pop();
///
/// println!("{}", owned);
/// ```
#[derive(Debug, Eq)]
pub struct String<'vm> {
    data: *const c_char,
    len: usize,
    phantom: PhantomData<&'vm VM<'vm>>,
}

impl String<'_> {
    /// Construct a new [String] starting from a pointer and a length to a J* `String`.
    pub(crate) fn new(data: *const c_char, len: usize) -> Self {
        String {
            data,
            len,
            phantom: PhantomData,
        }
    }

    /// Converts this J* string into a Rust [`&str`].
    ///
    /// As `String`s in J* can store arbitrary data and their encoding is not assumed, this method
    /// may return an utf8 encoding error.
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    /// Convers this [String] to a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: We know the vm is still valid (`self.phantom` lifetime). Also, as we have an
        // exclusive reference to the vm, we know the J* string couldn't have been possibly popped
        // from the stack, so we are guaranteed that the `data` pointer is still valid
        unsafe { std::slice::from_raw_parts(self.data as *const u8, self.len) }
    }
}

impl ToJStar for &str {
    /// Pushes a Rust `&str` onto the J* stack. See also [VM::push_string].
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl ToJStar for &[u8] {
    /// Pushes a Rust `&[u8]` onto the J* stack. See also [VM::push_string].
    fn to_jstar(&self, vm: &VM) {
        vm.push_string(self);
    }
}

impl ToJStar for String<'_> {
    /// Pushes this J* [String] onto the stack.  
    /// As the `String` is already owned by the VM, this method can skip a roundtrip through the
    /// J* stack and Rust, and directly push onto the J* stack, without copying the data.
    /// Also see [VM::push_value].
    fn to_jstar(&self, vm: &VM) {
        // TODO: welp, need to implement dup as documented above. Copying here is pretty stupid.
        vm.push_string(self.as_ref());
    }
}

impl ToJStar for &String<'_> {
    /// See [impl ToJStar for String<'_>](./struct.String.html#impl-ToJStar-for-String<'_>)
    fn to_jstar(&self, vm: &VM) {
        (*self).to_jstar(vm);
    }
}

impl<'vm> FromJStar<'vm> for String<'vm> {
    fn from_jstar(vm: &'vm VM, slot: Index) -> Option<Self> {
        vm.get_string(slot)
    }

    fn from_jstar_checked(vm: &'vm VM, slot: Index, name: &str) -> crate::error::Result<Self> {
        vm.check_string(slot, name)
    }
}

impl AsRef<[u8]> for String<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: AsRef<[u8]>> PartialEq<T> for String<'_> {
    fn eq(&self, other: &T) -> bool {
        self.as_bytes() == other.as_ref()
    }
}

impl Hash for String<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

impl From<String<'_>> for Vec<u8> {
    fn from(value: String<'_>) -> Self {
        value.as_bytes().to_vec()
    }
}

impl TryFrom<String<'_>> for std::string::String {
    type Error = std::str::Utf8Error;

    fn try_from(value: String<'_>) -> Result<Self, Self::Error> {
        value.as_str().map(std::string::String::from)
    }
}
