fn main() {
    println!("cargo:rustc-link-lib=LLVM-7");

    cc::Build::new()
        .file("wrappers/triple.cpp")
        .opt_level(3)
        .cpp(true)
        .flag("-std=c++14")
        .compile("triplewrappers");
}
