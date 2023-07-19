use crate::ffi;
use crate::vm::{ErrorCallback, ImportCallback};

#[derive(Default)]
pub struct Conf<'a> {
    pub starting_stack_sz: usize,
    pub first_gc_collection_point: usize,
    pub heap_grow_rate: i32,
    pub error_callback: Option<ErrorCallback<'a>>,
    pub import_callback: Option<ImportCallback<'a>>,
}

impl<'a> Conf<'a> {
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

    pub fn starting_stack_sz(mut self, size: usize) -> Self {
        self.starting_stack_sz = size;
        self
    }

    pub fn first_gc_collection_point(mut self, collection_point: usize) -> Self {
        self.first_gc_collection_point = collection_point;
        self
    }

    pub fn heap_grow_rate(mut self, rate: i32) -> Self {
        self.heap_grow_rate = rate;
        self
    }

    pub fn error_callback(mut self, error_cb: ErrorCallback<'a>) -> Self {
        self.error_callback = Some(error_cb);
        self
    }

    pub fn import_callback(mut self, import_cb: ImportCallback<'a>) -> Self {
        self.import_callback = Some(import_cb);
        self
    }
}
