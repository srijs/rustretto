use std::fs::File;
use std::io::Write;

use assert_cli::Assert;
use tempfile::TempDir;

#[test]
fn println() {
    TestCase(
        r#"
        public class Test {
            public static void main(String[] args) {
                System.out.println("Hello, World!");
            }
        }"#,
    )
    .expect_output("Hello, World!\n");
}

#[test]
fn for_loop() {
    TestCase(
        r#"
        public class Test {
            public static void main(String[] args) {
                int i;
                for (i = 0; i < 3; i++) {
                    System.out.println("X");
                }
            }
        }"#,
    )
    .expect_output("X\nX\nX\n");
}

#[test]
fn if_else() {
    TestCase(
        r#"
        public class Test {
            static void print(boolean condition) {
                if (condition) {
                    System.out.println("It's true!");
                } else {
                    System.out.println("False :(");
                }
            }

            public static void main(String[] args) {
                print(true);
              	print(false);
            }
        }"#,
    )
    .expect_output("It's true!\nFalse :(\n");
}

#[test]
fn inheritance() {
    TestCase(
        r#"
        public class Test {
            public static void main(String[] args) {
                class A {
                    public void printName() {
                        System.out.println("A");
                    }
                }

                class B extends A {
                    @Override
                    public void printName() {
                        System.out.println("B");
                    }
                }

                A a = new A();
                B b = new B();

                ((A)a).printName();
                ((A)b).printName();
            }
        }"#,
    )
    .expect_output("A\nB\n")
}

struct TestCase(&'static str);

impl TestCase {
    fn expect_output(&self, expected: &'static str) {
        let cwd = std::env::current_dir().unwrap();

        let tmpdir = TempDir::new().unwrap();
        let tmppath = tmpdir.path();

        let runtime_path = cwd.join("../target/release/libruntime.a");
        let output_path = tmppath.join("Test");

        let mut srcfile = File::create(tmppath.join("Test.java")).unwrap();
        srcfile.write_all(self.0.as_bytes()).unwrap();
        srcfile.sync_all().unwrap();

        Assert::command(&["javac", "Test.java"])
            .current_dir(&tmppath)
            .unwrap();

        let mut classes = vec![];
        for entry_result in tmppath.read_dir().unwrap() {
            let entry = entry_result.unwrap();
            let path = entry.path();
            let is_class = path.extension().map(|ext| ext == "class").unwrap_or(false);
            if is_class {
                classes.push(path);
            }
        }

        Assert::cargo_binary("compiler")
            .with_args(&["-r"])
            .with_args(&[&runtime_path])
            .with_args(&["-o"])
            .with_args(&[&output_path])
            .with_args(&["--main", "Test"])
            .with_args(&classes)
            .unwrap();

        Assert::command(&[output_path])
            .stdout()
            .is(expected)
            .unwrap();
    }
}
