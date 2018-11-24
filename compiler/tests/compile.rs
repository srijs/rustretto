extern crate assert_cli;
extern crate tempfile;

use std::fs::File;
use std::io::Write;

use assert_cli::Assert;
use tempfile::TempDir;

fn test_case(source: &str, expected: &'static str) {
    let cwd = std::env::current_dir().unwrap();

    let tmpdir = TempDir::new().unwrap();
    let tmppath = tmpdir.path();

    let runtime_path = cwd.join("../target/release/libruntime.a");
    let output_path = tmppath.join("Test");

    let mut srcfile = File::create(tmppath.join("Test.java")).unwrap();
    srcfile.write_all(source.as_bytes()).unwrap();
    srcfile.sync_all().unwrap();

    Assert::command(&["javac", "Test.java"])
        .current_dir(&tmppath)
        .unwrap();

    Assert::cargo_binary("compiler")
        .with_args(&["-r"])
        .with_args(&[&runtime_path])
        .with_args(&["-o"])
        .with_args(&[&output_path])
        .with_args(&[tmppath.join("Test.class")])
        .unwrap();

    Assert::command(&[output_path])
        .stdout()
        .is(expected)
        .unwrap();
}

#[test]
fn hello_world() {
    test_case(
        r#"
        public class Test {
            public static void main(String[] args) {
                System.out.println("Hello, World!");
            }
        }"#,
        "Hello, World!\n",
    );
}
