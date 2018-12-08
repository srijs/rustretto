use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use classfile::ClassFile;
use failure::Fallible;
use llvm;

use classes::ClassGraph;
use compile::Compiler;
use generate::CodeGen;
use loader::BootstrapClassLoader;

pub(crate) struct Driver {
    loader: BootstrapClassLoader,
    temppath: PathBuf,
    target: String,
    optimize: bool,
    modules: HashMap<String, String>,
}

impl Driver {
    pub fn new(home: PathBuf, target: String, optimize: bool, temppath: &Path) -> Fallible<Self> {
        let loader = BootstrapClassLoader::open(home)?;
        let modules = HashMap::new();
        Ok(Driver {
            loader,
            temppath: temppath.into(),
            target,
            optimize,
            modules,
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

        let codegen = CodeGen::new(classes.clone(), self.target.clone());
        let mut compiler = Compiler::new(classes.clone(), codegen);

        for class_name in class_names {
            let module = compiler.compile(&class_name, &*class_name == main)?;
            self.modules.insert(class_name.to_string(), module);
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

        let mut machine_builder = llvm::codegen::TargetMachineBuilder::host();
        if self.optimize {
            machine_builder.optimize(llvm::codegen::OptLevel::Aggressive);
        } else {
            machine_builder.optimize(llvm::codegen::OptLevel::None);
        }
        let machine = machine_builder.build();

        pass_manager.run(&mut main);
        let main_out = self.temppath.join("main.o");
        machine.emit_to_file(&main, llvm::codegen::FileType::Object, &main_out)?;

        let mut cmd = Command::new("ld");
        cmd.arg(main_out);
        cmd.arg(runtime_path);
        cmd.arg("-lc");
        cmd.arg("-o");
        cmd.arg(output_path);

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
