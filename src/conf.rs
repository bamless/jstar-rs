use jstar_sys::{JStarLoc, JStarRealloc};

use crate::{error::Error, ffi, import::Module, vm::VM};

pub(crate) type ErrorCallback<'a> = Box<dyn FnMut(Error, &str, Option<JStarLoc>, &str) + 'a + Send>;
pub(crate) type ImportCallback<'a> = Box<dyn FnMut(&mut VM, &str) -> Option<Module> + 'a + Send>;

// The realloc callback is a bare `extern "C"` function pointer — unlike the error and import
// callbacks it cannot be a Rust closure.  The C side holds no user-data pointer alongside it, and
// the very first call allocates the `JStarVM` struct itself, so neither the VM nor `custom_data`
// (Trampolines) exists yet.  Users who need custom allocation must write an `extern "C"` function.
pub(crate) type ReallocCallback = JStarRealloc;

/// Struct containing a set of configurations for the J* vm.
pub struct Conf<'a> {
    /// The initial stack size of the vm (in bytes)
    pub starting_stack_sz: usize,
    /// Threshold at which the first GC collection will happen (in bytes)
    pub first_gc_collection_point: usize,
    /// The rate at which the heap will grow after a GC pass
    pub heap_grow_rate: i32,
    /// Function called when an error occurs
    pub error_callback: Option<ErrorCallback<'a>>,
    /// Function called to resolve a module
    pub import_callback: Option<ImportCallback<'a>>,
    /// Function called to (re)allocate memory
    pub realloc: Option<ReallocCallback>,
}

impl<'a> Conf<'a> {
    /// Construct a new `Conf` struct with default values (equivalent of `jsrGetConf`).
    pub fn new() -> Self {
        let jstar_conf = ffi::JStarConf::default();
        Conf {
            starting_stack_sz: jstar_conf.starting_stack_sz,
            first_gc_collection_point: jstar_conf.first_gc_collection_point,
            heap_grow_rate: jstar_conf.heap_grow_rate,
            error_callback: None,
            import_callback: None,
            realloc: None,
        }
    }

    /// Set the starting stack size and returns self for chaining
    pub fn starting_stack_sz(mut self, size: usize) -> Self {
        self.starting_stack_sz = size;
        self
    }

    /// Set the gc collection point returns self for chaining
    pub fn first_gc_collection_point(mut self, collection_point: usize) -> Self {
        self.first_gc_collection_point = collection_point;
        self
    }

    /// Set the heap grow rate returns self for chaining
    pub fn heap_grow_rate(mut self, rate: i32) -> Self {
        self.heap_grow_rate = rate;
        self
    }

    /// Set the callback invoked by the VM when there is an error to report.
    ///
    /// # Arguments
    ///
    /// * `err`  - An [`Error`] describing the type of error being reported.
    /// * `file` - The path of the J* module reporting the error.
    /// * `loc`  - The source location of the error, or `None` if unavailable.
    /// * `msg`  - A formatted message describing the error.
    pub fn error_callback(
        mut self,
        error_cb: impl FnMut(Error, &str, Option<JStarLoc>, &str) + 'a + Send,
    ) -> Self {
        self.error_callback = Some(Box::new(error_cb));
        self
    }

    /// Set the callback invoked by the VM to resolve an `import` statement.
    ///
    /// # Arguments
    ///
    /// * `vm`          - The J* [`VM`].
    /// * `module_name` - The full dotted path of the module being imported, as it appears in
    ///   the J* source.
    ///
    /// # Returns
    ///
    /// `Some(`[`Module`]`)` if the module was found, `None` to let J* fall back to its default
    /// resolution strategy.
    pub fn import_callback(
        mut self,
        import_cb: impl FnMut(&mut VM, &str) -> Option<Module> + 'a + Send,
    ) -> Self {
        self.import_callback = Some(Box::new(import_cb));
        self
    }

    /// Set the allocator used for all VM memory and returns self for chaining.
    ///
    /// Unlike the other callbacks, this must be a plain `extern "C"` function pointer — closures
    /// are not supported. The C side holds no user-data pointer alongside it, and the very first
    /// call allocates the `JStarVM` struct itself, before any Rust-side state exists.
    ///
    /// # Arguments
    ///
    /// * `ptr`     - Pointer to reallocate, or null for a fresh allocation.
    /// * `old_sz`  - Previous allocation size in bytes (0 when `ptr` is null).
    /// * `new_sz`  - Requested size in bytes (0 to free).
    ///
    /// # Returns
    ///
    /// A pointer to the (re)allocated memory, or null on failure / free.
    pub fn realloc(mut self, realloc: extern "C" fn(*mut (), usize, usize) -> *mut ()) -> Self {
        self.realloc = Some(realloc);
        self
    }
}

impl Default for Conf<'_> {
    fn default() -> Self {
        Conf::new()
    }
}
