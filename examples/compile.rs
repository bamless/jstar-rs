use jstar::{conf::Conf, error::Result, import::Module, vm::VM};

fn main() -> Result<()> {
    let conf = Conf::new()
        .error_callback(|_, file, loc, msg| {
            if let Some(loc) = loc {
                eprintln!("{file}:{}:{}: error", loc.line, loc.col);
            } else {
                eprintln!("{file}: error");
            }
            eprintln!("{msg}");
        })
        .import_callback(|vm, module_name| {
            if module_name == "binary" {
                let code = vm
                    .compile_in_memory("<bin>", "print('Compiled code!')")
                    .ok()?;
                Some(Module::binary(code, "<bin>".to_owned()))
            } else {
                None
            }
        });

    let vm = VM::new(conf).init_runtime();
    vm.eval("<string>", "import binary")?;

    Ok(())
}
