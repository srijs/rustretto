use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use classfile::ClassFile;
use failure::{bail, Fallible};
use llvm;
use platforms::Platform;

use crate::classes::ClassGraph;
use crate::compile::Compiler;
use crate::generate::CodeGen;
use crate::loader::BootstrapClassLoader;
use crate::target::Target;

pub(crate) struct Driver {
    loader: BootstrapClassLoader,
    target: Target,
    optimize: bool,
    modules: HashMap<String, String>,
    machine: Arc<llvm::codegen::TargetMachine>,
}

impl Driver {
    pub fn new(home: PathBuf, platform: Platform, optimize: bool) -> Fallible<Self> {
        let loader = BootstrapClassLoader::open(home)?;
        let target = Target::new(platform);
        let modules = HashMap::new();

        let mut machine_builder = llvm::codegen::TargetMachine::builder();
        machine_builder.set_reloc_mode(llvm::codegen::RelocMode::PIC);
        if optimize {
            machine_builder.set_opt_level(llvm::codegen::OptLevel::Aggressive);
        }
        let machine = Arc::new(machine_builder.build()?);

        Ok(Driver {
            loader,
            target,
            optimize,
            modules,
            machine,
        })
    }

    pub fn compile(&mut self, main: &str, inputs: &[PathBuf]) -> Fallible<()> {
        let classes = ClassGraph::new(self.loader.clone());

        let mut class_names = vec![];
        for input in inputs {
            let file = fs::File::open(input)?;
            let class_file = ClassFile::parse(file)?;
            let class_name = class_file.get_name().clone();

            classes.add(class_file);
            class_names.push(class_name);
        }

        let codegen = CodeGen::new(classes.clone(), self.machine.clone())?;
        let mut compiler = Compiler::new(classes.clone(), codegen);

        for class_name in class_names {
            let module = compiler.compile(&class_name, &*class_name == main)?;
            self.modules.insert(class_name.to_string(), module);
        }

        Ok(())
    }

    pub fn dump(&self, path: &Path) -> Fallible<()> {
        for (name, module) in self.modules.iter() {
            let filename = format!("{}.ll", name.replace("/", "_"));
            let mut file = fs::File::create(path.join(filename))?;
            file.write_all(module.as_bytes())?;
        }
        Ok(())
    }

    pub fn link(&self, runtime_path: &Path, output_path: &Path) -> Fallible<()> {
        let mut main = llvm::Module::new("main");

        for (_name, module) in self.modules.iter() {
            main.link(llvm::Module::parse_ir(module.as_bytes())?)?;
        }

        let mut pass_manager_builder = llvm::transform::PassManagerBuilder::new();
        if self.optimize {
            pass_manager_builder.set_opt_level(llvm::transform::OptLevel::O3);
        } else {
            pass_manager_builder.set_opt_level(llvm::transform::OptLevel::O0);
        }
        let pass_manager = pass_manager_builder.build();

        pass_manager.run(&mut main);
        let main_obj = self
            .machine
            .emit_to_buffer(&main, llvm::codegen::FileType::Object)?;
        let mut main_out = tempfile::Builder::new().suffix(".o").tempfile()?;
        main_out.write_all(&main_obj)?;
        main_out.flush()?;

        let mut cmd = Command::new("/usr/bin/cc");
        cmd.arg(main_out.path());
        cmd.arg(runtime_path);
        cmd.arg("-lc");
        cmd.arg("-o");
        cmd.arg(output_path);

        match self.target.os() {
            "macos" => {
                let triple = self.machine.triple();
                let (major, minor, micro) = triple.get_macosx_version();
                cmd.arg(format!(
                    "-mmacosx-version-min={}.{}.{}",
                    major, minor, micro
                ));
            }
            _ => {}
        };

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
