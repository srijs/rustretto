fn main() {
    println!("cargo:rustc-link-lib=LLVM-7");

    cc::Build::new()
        .file("wrappers/target.c")
        .opt_level(3)
        .compile("targetwrappers");

    cc::Build::new()
        .file("wrappers/triple.cpp")
        .opt_level(3)
        .cpp(true)
        .flag("-std=c++14")
        .compile("triplewrappers");
}
