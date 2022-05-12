fn asm(out_dir: &str) {
    use std::process::Command;

    println!("cargo:rerun-if-changed=src/arch/x86_64/trampoline.asm");

    let status = Command::new("nasm")
        .arg("-f").arg("bin")
        .arg("-o").arg(format!("{}/trampoline", out_dir))
        .arg("src/arch/x86_64/trampoline.asm")
        .status()
        .expect("Could not run nasm");

        if !status.success() {
            panic!("nasm failed with exit status: {}", status)
        }
}

fn main() {
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());

    let out_dir = std::env::var("OUT_DIR").unwrap();
    asm(&out_dir);


}
