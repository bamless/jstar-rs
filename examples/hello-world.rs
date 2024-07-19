use jstar::{self, conf::Conf, error::Result, vm::VM};

fn main() -> Result<()> {
    let vm = VM::new(Conf::new()).init_runtime();
    vm.eval("<string>", "print('Hello from Rust 🦀!')")?;
    Ok(())
}
