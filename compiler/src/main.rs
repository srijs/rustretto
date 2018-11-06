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

use classfile::attrs::stack_map_table::VerificationTypeInfo;
use classfile::attrs::Code;
use classfile::descriptors::ParameterDescriptor;
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
mod utils;

use classes::ClassGraph;
use frame::StackAndLocals;
use loader::{BootstrapClassLoader, Class};
use translate::{Type, VarIdGen};

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

    let cf = match classes.get(&class_name).unwrap() {
        Class::File(class_file) => class_file,
        class => bail!("unexpected class type {:?}", class),
    };

    generate::gen_prelude(cf);
    for method in cf.methods.iter() {
        let mut var_id_gen = VarIdGen::new();
        let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
        let mut args = Vec::new();
        if name == "<init>" {
            let arg_type = Type {
                info: VerificationTypeInfo::UninitializedThis,
            };
            args.push(var_id_gen.gen(arg_type));
        } else {
            let arg_type = Type {
                info: VerificationTypeInfo::Object(class_name.to_owned()),
            };
            args.push(var_id_gen.gen(arg_type));
        }
        for ParameterDescriptor::Field(field_type) in method.descriptor.params.iter() {
            args.push(var_id_gen.gen(Type::from_field_type(field_type.clone())));
        }
        let code = method.attributes.get::<Code>().unwrap();
        let state = StackAndLocals::new(code.max_stack, code.max_locals, &args);
        let blocks = translate::translate_method(
            code.disassemble(),
            state,
            &cf.constant_pool,
            &mut var_id_gen,
        )?;
        generate::gen_method(&cf, &method, &blocks, &cf.constant_pool, &mut var_id_gen);
    }
    generate::gen_main(&cf);
    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(err) = compile(Compile::from_args()) {
        println!("Error: {:?}", err);
    }
}
