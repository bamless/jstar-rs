use jstar::{conf::Conf, error::Result, import::Module, vm::VM};

fn main() -> Result<()> {
    let conf = Conf::new()
        .error_callback(Box::new(|_, file, line, msg| {
            if let Some(line) = line {
                eprintln!("Error {file} [line:{line}]:");
            } else {
                eprintln!("Error {file}:");
            }
            eprintln!("{msg}");
        }))
        .import_callback(Box::new(|vm, module_name| {
            if module_name == "binary" {
                let code = vm
                    .compile_in_memory("<bin>", "print('Compiled code!')")
                    .ok()?;
                Some(Module::binary(code, "<bin>".to_owned()))
            } else {
                None
            }
        }));

    let vm = VM::new(conf);
    let mut vm = vm.init_runtime();
    vm.eval_string("<string>", "import binary")?;

    Ok(())
}
