use jstar::{self, conf::ConfBuilder, error::Result, vm::NewVM};

fn main() -> Result<()> {
    let vm = NewVM::new(ConfBuilder::default().build());
    let mut vm = vm.init_runtime();

    vm.eval_string("<string>", "print('Hello from Rust 🦀!')")?;

    Ok(())
}
