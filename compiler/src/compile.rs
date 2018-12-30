use classfile::attrs::Code;
use classfile::constant_pool::Constant;
use classfile::descriptors::ParameterDescriptor;
use failure::{bail, Fallible};
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::frame::StackAndLocals;
use frontend::loader::Class;
use frontend::translate::{self, VarIdGen};
use frontend::types::Type;

use backend::CodeGen;

pub struct Compiler {
    classes: ClassGraph,
    codegen: CodeGen,
}

impl Compiler {
    pub fn new(classes: ClassGraph, codegen: CodeGen) -> Self {
        Self { classes, codegen }
    }

    pub fn compile(&mut self, class_name: &StrBuf, main: bool) -> Fallible<String> {
        let cf = match self.classes.get(&class_name)? {
            Class::File(class_file) => class_file,
            class => bail!("unexpected class type {:?}", class),
        };

        let mut classgen = self.codegen.generate_class(class_name)?;

        classgen.gen_prelude()?;

        for index in cf.constant_pool.indices() {
            if let Constant::Class(class_const) = cf.constant_pool.get_info(index).unwrap() {
                let ext_class_name = cf.constant_pool.get_utf8(class_const.name_index).unwrap();
                // don't emit external declarations for own class
                if ext_class_name == class_name {
                    continue;
                }
                if let Class::File(ext_class_file) = self.classes.get(ext_class_name)? {
                    classgen.gen_extern_decls(&ext_class_file)?;
                    classgen.gen_object_type(ext_class_name)?;
                    classgen.gen_vtable_type(ext_class_name)?;
                }
            }
        }

        classgen.gen_object_type(class_name)?;
        classgen.gen_vtable_type(class_name)?;
        classgen.gen_vtable_decls(class_name)?;
        classgen.gen_vtable_const(class_name)?;

        for method in cf.methods.iter() {
            let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
            log::debug!("compiling method {} of class {}", name, class_name);

            let mut args = Vec::new();
            let mut var_id_gen = VarIdGen::default();
            if &**name == "<init>" || !method.is_static() {
                let arg_type = Type::Reference;
                args.push(var_id_gen.gen(arg_type));
            }
            for ParameterDescriptor::Field(field_type) in method.descriptor.params.iter() {
                args.push(var_id_gen.gen(Type::from_field_type(field_type)));
            }

            if method.is_native() {
                classgen.gen_native_method(&method, &args, &cf.constant_pool)?;
                continue;
            }

            let code = method.attributes.get::<Code>().unwrap();
            let state = StackAndLocals::new(code.max_stack, code.max_locals, &args);
            let blocks = translate::translate_method(
                code.disassemble(),
                state,
                &cf.constant_pool,
                &mut var_id_gen,
            )?;
            classgen.gen_method(&method, &blocks, &cf.constant_pool)?;

            if &**name == "<clinit>" {
                classgen.gen_class_init()?;
            }
        }

        if main {
            classgen.gen_main()?;
        }

        Ok(classgen.finish())
    }
}
