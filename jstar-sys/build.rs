use cmake::Config;
use glob::glob;
use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let vendored = env::var("CARGO_FEATURE_VENDORED").is_ok();
    let statik = env::var("JSTAR_STATIC").is_ok() || env::var("CARGO_FEATURE_STATIC").is_ok();

    if !vendored {
        let mut cfg = pkg_config::Config::new();
        if let Ok(lib) = cfg.atleast_version("1.9").statik(statik).probe("jstar") {
            for path in &lib.include_paths {
                println!("cargo:include={}", path.display());
            }
            return;
        }
    }

    println!("cargo:rustc-cfg=vendored");

    if !Path::new("jstar/.git").exists() {
        Command::new("git")
            .args(["submodule", "update", "--init", "jstar"])
            .status()
            .unwrap();
    }

    let dst = Config::new("jstar").build();
    for entry in glob(format!("{}/lib*/", dst.display()).as_str()).unwrap() {
        println!(
            "cargo:rustc-link-search=native={}",
            entry.unwrap().display()
        );
    }
    println!(
        "cargo:rustc-link-lib={}=jstar",
        if statik { "static" } else { "dylib" }
    );

    println!("cargo:rerun-if-changed=jstar/include");
    println!("cargo:rerun-if-changed=jstar/src");
    println!("cargo:rerun-if-changed=jstar/extern");
}
