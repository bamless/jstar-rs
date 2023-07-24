use jstar::{conf::Conf, import::Module, vm::VM, error::Result};

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
        .import_callback(Box::new(|_, module_name| {
            if module_name == "hello_world" {
                let code = "
                fun hello()
                    print('Hello from Rust 🦀!')
                end
                ";

                Some(Module::source(
                    code.to_owned(),
                    "hello_world.jsr".to_owned(),
                ))
            } else {
                None
            }
        }));

    let vm = VM::new(conf);
    let vm = vm.init_runtime();

    vm.eval_string(
        "<string>",
        "import hello_world for hello
        hello()",
    )?;

    println!();

    vm.eval_string("<string>", "import does_not_exist")?;

    Ok(())
}
