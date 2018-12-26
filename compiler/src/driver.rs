use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use classfile::ClassFile;
use failure::{bail, Fallible};
use llvm;
use target_lexicon::{OperatingSystem, Triple};

use frontend::classes::ClassGraph;
use frontend::loader::{BootstrapClassLoader, InputClassLoader};

use backend::{CodeGen, Target};

use crate::compile::Compiler;

pub struct Driver {
    loader: BootstrapClassLoader,
    target_triple: Triple,
    optimize: bool,
    modules: HashMap<String, String>,
    machine: Arc<llvm::codegen::TargetMachine>,
}

impl Driver {
    pub fn new(home: PathBuf, target_triple: Triple, optimize: bool) -> Fallible<Self> {
        let loader = BootstrapClassLoader::open(home)?;
        let modules = HashMap::new();

        let mut machine_builder = llvm::codegen::TargetMachine::builder();
        machine_builder.set_reloc_mode(llvm::codegen::RelocMode::PIC);
        if optimize {
            machine_builder.set_opt_level(llvm::codegen::OptLevel::Aggressive);
        }
        let machine = Arc::new(machine_builder.build()?);

        Ok(Driver {
            loader,
            target_triple,
            optimize,
            modules,
            machine,
        })
    }

    pub fn compile(&mut self, main: &str, inputs: &[PathBuf]) -> Fallible<()> {
        let mut loader = InputClassLoader::new(self.loader.clone());

        let mut class_names = vec!["java/lang/Object".to_owned().into()];
        for input in inputs {
            let file = fs::File::open(input)?;
            let class_file = ClassFile::parse(file)?;
            let class_name = class_file.get_name().clone();

            loader.add_input(class_file);
            class_names.push(class_name);
        }

        let classes = ClassGraph::new(loader);
        let target = Target {
            triple: self.machine.triple().to_string(),
            data_layout: self.machine.data_layout().to_string_rep().to_string(),
        };
        let codegen = CodeGen::new(classes.clone(), target)?;
        let mut compiler = Compiler::new(classes.clone(), codegen);

        for class_name in class_names {
            let module = compiler.compile(&class_name, &*class_name == main)?;
            self.modules.insert(class_name.to_string(), module);
        }

        Ok(())
    }

    pub fn dump(&self, path: &Path) -> Fallible<()> {
        for (name, module) in self.modules.iter() {
            let filename = format!("{}.ll", name.replace("/", "."));
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

        let mut cmd = Command::new("cc");

        if cfg!(target_os = "macos") {
            // hack: clear dyld library path to make `cargo test` work on mac
            cmd.env_remove("DYLD_LIBRARY_PATH");
        }

        cmd.arg(main_out.path());
        cmd.arg(runtime_path);
        cmd.arg("-o");
        cmd.arg(output_path);
        cmd.args(&["-lpthread", "-ldl"]);

        match self.target_triple.operating_system {
            OperatingSystem::Darwin => {
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
