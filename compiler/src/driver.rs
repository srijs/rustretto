use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use classfile::ClassFile;
use failure::Fallible;
use tempfile::TempDir;

use classes::ClassGraph;
use compile::Compiler;
use generate::CodeGen;
use loader::BootstrapClassLoader;

pub(crate) struct Driver {
    loader: BootstrapClassLoader,
    temppath: PathBuf,
    target: String,
    optimize: u32
}

impl Driver {
    pub fn new(home: PathBuf, target: String, optimize: u32, temppath: &Path) -> Fallible<Self> {
        let loader = BootstrapClassLoader::open(home)?;
        let tmpdir = TempDir::new()?;
        Ok(Driver {
            loader,
            temppath: temppath.into(),
            target,
            optimize
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

        let codegen = CodeGen::new(self.temppath.clone(), self.target.clone());
        let mut compiler = Compiler::new(classes, codegen);

        compiler.compile(&class_name)
    }

    pub fn link(&self, runtime_path: &Path, output_path: &Path) -> Fallible<()> {
        let mut files = vec![];
        for entry_result in self.temppath.read_dir()? {
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

        // configure output
        cmd.arg("-o");
        cmd.arg(output_path);

        // configure optimizations
        match self.optimize {
            0 => cmd.arg("-O0"),
            1 => cmd.arg("-01"),
            2 => cmd.arg("-02"),
            3 => cmd.args(&["-O3", "-flto"]),
            x => bail!("unknown optimization level {}", x)
        };

        // configure inputs
        cmd.arg(runtime_path);
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
