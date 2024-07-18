use jstar::{
    self, conf::Conf, convert::FromJStar, error::Result, native, string::String, vm::VM,
    MAIN_MODULE,
};

native!(fn nativePrint(vm) {
    let str = String::from_jstar_checked(vm, 1, "str")?;
    println!("✶{}✶", str.as_str().unwrap());
    Ok(())
});

fn main() -> Result<()> {
    let conf = Conf::new().error_callback(Box::new(|_, file, line, msg| {
        if let Some(line) = line {
            eprintln!("Error {file} [line:{line}]:");
        } else {
            eprintln!("Error {file}:");
        }
        eprintln!("{msg}");
    }));

    let vm = VM::new(conf).init_runtime();
    vm.register_native(MAIN_MODULE, "nativePrint", nativePrint, 1).unwrap();
    vm.eval_string("<string>", "nativePrint('🦀')")?;

    Ok(())
}
