use jstar::{self, conf::Conf, error::Result, vm::VM};
use std::io::{self, BufRead, Write};

fn main() -> Result<()> {
    let conf = Conf::new().error_callback(Box::new(|_, file, line, msg| {
        if let Some(line) = line {
            eprintln!("Error {file} [line:{line}]:");
        } else {
            eprintln!("Error {file}:");
        }
        eprintln!("{msg}");
    }));

    let vm = VM::new(conf);
    let mut vm = vm.init_runtime();

    let mut stdin = io::stdin().lock();
    loop {
        print!("J*>> ");
        io::stdout().flush()?;

        let mut line = String::new();
        let nread = stdin.read_line(&mut line)?;
        if nread == 0 {
            break;
        }

        let _ = vm.eval_string("<repl>", &line);
    }

    Ok(())
}
