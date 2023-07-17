use jstar::{
    self, conf::ConfBuilder, convert::FromJStar, convert::ToJStar, error::Result, vm::NewVM,
};

fn main() -> Result<()> {
    let vm = NewVM::new(ConfBuilder::default().build());
    let mut vm = vm.init_runtime();

    10.0.to_jstar(&mut vm);
    let res = f64::from_jstar(&vm, -1).unwrap();
    println!("{res}");

    50.0.to_jstar(&mut vm);
    let sref = vm.get_top();
    let sref2 = vm.get_top();

    let n: f64 = sref.get().unwrap();
    let n2: f64 = sref2.get().unwrap();

    vm.eval_string("<string>", "print('ciao')").unwrap();

    println!("{n} {n2}");

    Ok(())
}
