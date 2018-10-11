extern crate chrono;
extern crate classfile;
extern crate failure;
extern crate md5;
#[macro_use]
extern crate structopt;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use classfile::{attrs, instructions::Instr, ClassFile};
use failure::Fallible;
use structopt::StructOpt;

fn format_constant(idx: u16, pool: &classfile::ConstantPool) -> String {
    use classfile::constant_pool::{Constant, ConstantIndex};

    match pool.get_info(ConstantIndex::from_u16(idx)).unwrap() {
        Constant::FieldRef(field_ref) => {
            let class = pool.get_class(field_ref.class_index).unwrap();
            let class_name = pool.get_utf8(class.name_index).unwrap();
            let name_and_type = pool
                .get_name_and_type(field_ref.name_and_type_index)
                .unwrap();
            let name = pool.get_utf8(name_and_type.name_index).unwrap();
            let descriptor = pool.get_utf8(name_and_type.descriptor_index).unwrap();
            format!("Field {}.{}:{}", class_name, name, descriptor)
        }
        Constant::MethodRef(method_ref) => {
            let class = pool.get_class(method_ref.class_index).unwrap();
            let class_name = pool.get_utf8(class.name_index).unwrap();
            let name_and_type = pool
                .get_name_and_type(method_ref.name_and_type_index)
                .unwrap();
            let name = pool.get_utf8(name_and_type.name_index).unwrap();
            let descriptor = pool.get_utf8(name_and_type.descriptor_index).unwrap();
            if name == "<init>" {
                format!("Method {}.\"<init>\":{}", class_name, descriptor)
            } else {
                format!("Method {}.{}:{}", class_name, name, descriptor)
            }
        }
        Constant::String(string) => {
            format!("String {}", pool.get_utf8(string.string_index).unwrap())
        }
        c => format!("{:?}", c),
    }
}

fn format_instr(ipos: u32, instr: &Instr, pool: &classfile::ConstantPool) -> String {
    match instr {
        Instr::ALoad0 => format!("aload_0"),
        Instr::InvokeSpecial(n) => {
            format!("invokespecial #{:<19}// {}", n, format_constant(*n, pool))
        }
        Instr::Return => format!("return"),
        Instr::IConst0 => format!("iconst_0"),
        Instr::InvokeStatic(n) => {
            format!("invokestatic  #{:<19}// {}", n, format_constant(*n, pool))
        }
        Instr::AStore1 => format!("astore_1"),
        Instr::ALoad1 => format!("aload_1"),
        Instr::InvokeVirtual(n) => {
            format!("invokevirtual #{:<19}// {}", n, format_constant(*n, pool))
        }
        Instr::IfEq(off) => format!("ifeq          {}", ipos as i64 + *off as i64),
        Instr::GetStatic(n) => format!("getstatic     #{:<19}// {}", n, format_constant(*n, pool)),
        Instr::LdC(n) => format!(
            "ldc           #{:<19}// {}",
            n,
            format_constant(*n as u16, pool)
        ),
        _ => format!("{:?}", instr),
    }
}

fn format_field_type(field_type: &classfile::descriptors::FieldType, out: &mut String) {
    use classfile::descriptors::*;

    match field_type {
        &FieldType::Base(ref base_type) => match base_type {
            BaseType::Byte => out.push_str("byte"),
            BaseType::Char => out.push_str("char"),
            BaseType::Double => out.push_str("double"),
            BaseType::Float => out.push_str("float"),
            BaseType::Int => out.push_str("int"),
            BaseType::Long => out.push_str("long"),
            BaseType::Short => out.push_str("short"),
            BaseType::Boolean => out.push_str("boolean"),
        },
        &FieldType::Array(ArrayType { ref component_type }) => {
            format_field_type(component_type, out);
            out.push_str("[]");
        }
        &FieldType::Object(ObjectType { ref class_name }) => {
            out.push_str(class_name);
        }
    }
}

fn format_method_parameters(desc: &classfile::MethodDescriptor, out: &mut String) {
    use classfile::descriptors::*;

    out.push('(');
    for ParameterDescriptor::Field(ref field_type) in desc.params.iter() {
        format_field_type(field_type, out);
    }
    out.push(')');
}

fn format_method(
    this_class_name: &str,
    method: &classfile::Method,
    consts: &classfile::ConstantPool,
    out: &mut String,
) {
    let access_flags = method.access_flags;
    if access_flags.contains(classfile::MethodAccessFlags::PUBLIC) {
        out.push_str("public ");
    }
    if access_flags.contains(classfile::MethodAccessFlags::STATIC) {
        out.push_str("static ");
    }

    let method_name = consts.get_utf8(method.name_index).unwrap();
    if method_name == "<init>" {
        out.push_str(this_class_name);
    } else {
        match method.descriptor.ret {
            classfile::descriptors::ReturnTypeDescriptor::Field(ref field_type) => {
                format_field_type(field_type, out);
                out.push(' ');
            }
            classfile::descriptors::ReturnTypeDescriptor::Void => {
                out.push_str("void ");
            }
        }
        out.push_str(method_name);
    }

    format_method_parameters(&method.descriptor, out);

    out.push(';');
}

fn compute_md5<P: AsRef<Path>>(path: P) -> Fallible<md5::Digest> {
    let mut file = fs::File::open(path.as_ref())?;
    let mut ctx = md5::Context::new();
    let mut buffer = [0u8; 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n > 0 {
            ctx.consume(&buffer[..n]);
        } else {
            break;
        }
    }
    Ok(ctx.into())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "javapv")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

fn analyze(opt: Opt) -> Fallible<()> {
    let metadata = opt.input.metadata()?;
    let file = fs::File::open(&opt.input)?;
    let cf = ClassFile::parse(file)?;

    println!("Classfile {}", opt.input.canonicalize()?.display());
    println!(
        "  Last modified {}; size {} bytes",
        chrono::DateTime::<chrono::Local>::from(metadata.modified()?).format("%d/%m/%Y"),
        metadata.len()
    );
    println!("  MD5 checksum {:x}", compute_md5(&opt.input)?);

    let source_file = cf.attributes.get::<attrs::SourceFile>().unwrap();

    println!("  Compiled from {:?}", source_file.as_str());

    let access_flags = cf.access_flags;
    if access_flags.contains(classfile::ClassAccessFlags::PUBLIC) {
        print!("public ");
    }

    let this_class = cf.get_this_class();
    let this_class_name = cf.constant_pool.get_utf8(this_class.name_index).unwrap();

    println!("class {}", this_class_name);
    println!("  minor version: {}", cf.version.minor);
    println!("  major version: {}", cf.version.major);

    println!("Constant pool:");
    for idx in cf.constant_pool.indices() {
        println!(
            "{:>5} = {}",
            format!("#{}", idx.as_u16()),
            format_constant(idx.as_u16(), &cf.constant_pool)
        )
    }

    println!("{{");

    for (i, method) in cf.methods.iter().enumerate() {
        if i > 0 {
            println!("");
        }

        let mut formatted_method = String::new();
        format_method(
            this_class_name,
            &method,
            &cf.constant_pool,
            &mut formatted_method,
        );
        println!("  {}", formatted_method);

        if let Ok(code) = method.attributes.get::<attrs::Code>() {
            let mut args_size = method.descriptor.params.len();
            let method_name = cf.constant_pool.get_utf8(method.name_index).unwrap();
            if method_name == "<init>" {
                args_size += 1;
            }
            println!("    Code:");
            println!(
                "      stack={}, locals={}, args_size={}",
                code.max_stack, code.max_locals, args_size
            );
            let mut instructions = code.disassemble();
            while let Some((ipos, instr)) = instructions.decode_next()? {
                println!(
                    "    {:>4}: {}",
                    ipos,
                    format_instr(ipos, &instr, &cf.constant_pool)
                );
            }

            if let Ok(stack_map_table) = code.attributes.get::<attrs::StackMapTable>() {
                println!("    StackMapTable: {:?}", stack_map_table);
            }
        }
    }

    println!("}}");

    println!("SourceFile: {:?}", source_file.as_str());

    Ok(())
}

fn main() {
    let opt = Opt::from_args();

    analyze(opt).unwrap()
}
