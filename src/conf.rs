use crate::{error::Error, ffi, import::ImportResult, vm::VM};

/// Callback invoked by the J* vm when there is an error to report
///
/// # Arguments
///
/// * `err`  - An [Error] describing which type of error we are reporting.
/// * `file` - A string representing the file path of the module J* module reporting the error.
/// * `line` - The line at which the error occured, or `None` if there isn't one.
/// * `msg`  - A string that contains a formatted message describing the error.
pub type ErrorCallback<'a> = Box<dyn FnMut(Error, &str, Option<i32>, &str) + 'a>;

/// Callback invoked by the J* vm to resolve an `import` statement
///
/// # Arguments
///
/// * `vm`          - A J* [VM]
/// * `module_name` - A string that contains the full path of the import as it appears in the J*
///   code
///
/// # Returns
///
/// An [ImportResult] with the code of the resolved module if one can be found, `Err` otherwise.
pub type ImportCallback<'a> = Box<dyn FnMut(&mut VM, &str) -> ImportResult + 'a>;

/// Strutc containing a set of configurations for the J* vm.
#[derive(Default)]
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

    /// Set the error callback returns self for chaining
    pub fn error_callback(mut self, error_cb: ErrorCallback<'a>) -> Self {
        self.error_callback = Some(error_cb);
        self
    }

    /// Set the import callback returns self for chaining
    pub fn import_callback(mut self, import_cb: ImportCallback<'a>) -> Self {
        self.import_callback = Some(import_cb);
        self
    }
}
