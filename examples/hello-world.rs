use jstar::{self, conf::Conf, error::Result, vm::NewVM};

fn main() -> Result<()> {
    let vm = NewVM::new(Conf::new());
    let vm = vm.init_runtime();
    vm.eval_string("<string>", "print('Hello from Rust 🦀!')")?;
    Ok(())
}
