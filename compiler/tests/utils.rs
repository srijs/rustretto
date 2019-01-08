use std::fs::File;
use std::io::Write;

use assert_cli::Assert;
use serde_derive::Deserialize;
use tempfile::TempDir;

#[macro_export]
macro_rules! cases {
    ($($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                serde_yaml::from_str::<utils::TestCase>(include_str!(concat!("cases/", stringify!($name), ".yml")))
                    .unwrap()
                    .expect()
            }
        )*
    }
}

#[derive(Deserialize)]
pub struct TestCase {
    source: String,
    output: String,
}

impl TestCase {
    pub fn expect(&self) {
        let cwd = std::env::current_dir().unwrap();

        let tmpdir = TempDir::new().unwrap();
        let tmppath = tmpdir.path();

        let runtime_path = cwd.join("../runtime/libruntime.a");
        let output_path = tmppath.join("Test");

        let mut srcfile = File::create(tmppath.join("Test.java")).unwrap();
        srcfile.write_all(self.source.as_bytes()).unwrap();
        srcfile.sync_all().unwrap();

        Assert::command(&["javac", "-encoding", "utf8", "Test.java"])
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
            .is(self.output.as_str())
            .unwrap();
    }
}
