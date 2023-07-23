use jstar::{self, conf::Conf, convert::ToJStar, error::Result, string::String, vm::NewVM};

fn main() -> Result<()> {
    let vm = NewVM::new(Conf::new());
    let vm = vm.init_runtime();

    42.to_jstar(&vm);
    let r1 = vm.get_top();

    "string".to_jstar(&vm);
    let r2 = vm.get_top();

    let v1: f64 = r1.get().unwrap();
    let v2: String = r2.get().unwrap();

    println!("{} {}", v1, v2.as_str().unwrap());

    Ok(())
}
