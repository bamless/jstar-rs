use jstar::{
    self,
    conf::Conf,
    convert::{FromJStar, ToJStar},
    error::Result,
    vm::NewVM,
};

fn main() -> Result<()> {
    let vm = NewVM::new(Conf::new());
    let mut vm = vm.init_runtime();

    let n = 30.0;
    n.to_jstar(&vm);
    println!("f64: {}", f64::from_jstar(&vm, -1).unwrap());
    vm.pop();

    let n = 40.0f32;
    n.to_jstar(&vm);
    println!("f32: {}", f64::from_jstar(&vm, -1).unwrap());
    vm.pop();

    let n = 50i32;
    n.to_jstar(&vm);
    println!("i32: {}", f64::from_jstar(&vm, -1).unwrap());
    vm.pop();

    let n = 60u32;
    n.to_jstar(&vm);
    println!("u32: {}", f64::from_jstar(&vm, -1).unwrap());
    vm.pop();

    Ok(())
}
