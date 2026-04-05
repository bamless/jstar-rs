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
        .import_callback(|_, module_name| {
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
        });

    let vm = VM::new(conf).init_runtime();

    vm.eval(
        "<string>",
        "import hello_world for hello
        hello()",
    )?;

    println!();

    vm.eval("<string>", "import does_not_exist")?;

    Ok(())
}
