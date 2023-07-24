use jstar::{
    conf::Conf,
    error::Result,
    import::{Module, NotFound},
    vm::VM,
};

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
        .import_callback(Box::new(|vm, path| {
            if path == "binary" {
                let code = vm.compile_in_memory("<bin>", "print('Compiled code!')")?;
                Ok(Module::binary(code, path.to_owned()))
            } else {
                Err(NotFound.into())
            }
        }));

    let vm = VM::new(conf);
    let vm = vm.init_runtime();
    vm.eval_string("<string>", "import binary")?;

    Ok(())
}
