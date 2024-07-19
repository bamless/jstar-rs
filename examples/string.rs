use jstar::{
    self, conf::Conf, convert::FromJStar, convert::ToJStar, error::Result, string::String, vm::VM,
};

fn main() -> Result<()> {
    let vm = VM::new(Conf::default());
    let vm = vm.init_runtime();

    "string from rust".to_jstar(&vm);
    let s = String::from_jstar(&vm, -1).unwrap();
    let s = s.as_str().unwrap();
    println!("{s}");

    Ok(())
}
