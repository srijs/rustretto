fn main() {
    println!("cargo:rustc-link-lib=LLVM-7");

    cc::Build::new()
        .file("wrappers/target.c")
        .compile("targetwrappers");
}
