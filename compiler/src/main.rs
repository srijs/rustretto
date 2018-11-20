extern crate classfile;
extern crate jar;
#[macro_use]
extern crate failure;
extern crate petgraph;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate structopt;

use std::fs;
use std::path::PathBuf;

use classfile::ClassFile;
use failure::Fallible;
use structopt::StructOpt;

mod blocks;
mod classes;
mod disasm;
mod frame;
mod generate;
mod loader;
mod translate;
mod types;
mod utils;
mod compile;

use classes::ClassGraph;
use generate::CodeGen;
use loader::BootstrapClassLoader;
use compile::Compiler;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "compile",
    about = "Compile JVM classfiles into a native executable."
)]
struct Compile {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
    #[structopt(long = "jar", parse(from_os_str))]
    jars: Vec<PathBuf>,
}

fn compile(c: Compile) -> Fallible<()> {
    let file = fs::File::open(c.input)?;
    let class_file = ClassFile::parse(file)?;

    let class_name = {
        let class = class_file.get_this_class();
        class_file
            .constant_pool
            .get_utf8(class.name_index)
            .unwrap()
            .to_owned()
    };

    let loader = BootstrapClassLoader::open(&c.jars)?;
    let classes = ClassGraph::build(class_file, &loader)?;
    let codegen = CodeGen::new("target-jvm".into());
    let mut compiler = Compiler::new(classes, codegen);

    compiler.compile(&class_name)
}

fn main() {
    env_logger::init();
    if let Err(err) = compile(Compile::from_args()) {
        println!("Error: {:?}", err);
    }
}
