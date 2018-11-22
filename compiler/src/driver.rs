use std::fs;
use std::path::PathBuf;
use std::process::Command;

use classfile::ClassFile;
use failure::Fallible;
use tempdir::TempDir;

use classes::ClassGraph;
use compile::Compiler;
use generate::CodeGen;
use loader::BootstrapClassLoader;

pub(crate) struct Driver {
    loader: BootstrapClassLoader,
    tmpdir: TempDir,
    target: String,
}

impl Driver {
    pub fn new(home: PathBuf, target: String) -> Fallible<Self> {
        let loader = BootstrapClassLoader::open(home)?;
        let tmpdir = TempDir::new("target")?;
        Ok(Driver {
            loader,
            tmpdir,
            target,
        })
    }

    pub fn compile(&self, input: &PathBuf) -> Fallible<()> {
        let file = fs::File::open(input)?;
        let class_file = ClassFile::parse(file)?;

        let class_name = {
            let class = class_file.get_this_class();
            class_file
                .constant_pool
                .get_utf8(class.name_index)
                .unwrap()
                .to_owned()
        };

        let classes = ClassGraph::build(class_file, self.loader.clone())?;

        let codegen = CodeGen::new(self.tmpdir.path().into(), self.target.clone());
        let mut compiler = Compiler::new(classes, codegen);

        compiler.compile(&class_name)
    }

    pub fn link(&self) -> Fallible<()> {
        let mut files = vec![];
        for entry_result in self.tmpdir.path().read_dir()? {
            let entry = entry_result?;
            let path = entry.path();
            let is_ll = path.extension().map(|ext| ext == "ll").unwrap_or(false);
            if is_ll {
                files.push(path);
            }
        }

        let mut cmd = Command::new("clang");
        cmd.arg(&format!("--target={}", self.target));
        cmd.arg("-Wno-override-module");
        cmd.args(&["-O3", "-flto", "target/release/libruntime.a"]);
        for path in files {
            cmd.arg(path);
        }
        let exit = cmd.status()?;
        if !exit.success() {
            if let Some(code) = exit.code() {
                bail!("linker exited with status code {}", code);
            } else {
                bail!("linker was terminated by signal");
            }
        }

        Ok(())
    }
}
