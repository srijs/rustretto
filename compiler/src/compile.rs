use classfile::attrs::Code;
use classfile::constant_pool::Constant;
use classfile::descriptors::ParameterDescriptor;
use failure::{bail, Fallible};
use strbuf::StrBuf;

use crate::classes::ClassGraph;
use crate::frame::StackAndLocals;
use crate::generate::CodeGen;
use crate::loader::Class;
use crate::translate::{self, VarIdGen};
use crate::types::Type;

pub(crate) struct Compiler {
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
            match cf.constant_pool.get_info(index).unwrap() {
                Constant::Class(class_const) => {
                    let ext_class_name = cf.constant_pool.get_utf8(class_const.name_index).unwrap();
                    // don't emit external declarations for own class
                    if ext_class_name == class_name {
                        continue;
                    }
                    match self.classes.get(ext_class_name)? {
                        Class::File(ext_class_file) => {
                            classgen.gen_extern_decls(&ext_class_file)?;
                            classgen.gen_vtable_type(ext_class_name)?;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        classgen.gen_vtable_type(class_name)?;
        classgen.gen_vtable_decls(class_name)?;
        classgen.gen_vtable_const(class_name)?;

        for method in cf.methods.iter() {
            let mut var_id_gen = VarIdGen::new();
            let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
            let mut args = Vec::new();
            if &**name == "<init>" {
                let arg_type = Type::UninitializedThis;
                args.push(var_id_gen.gen(arg_type));
            } else if !method.is_static() {
                let arg_type = Type::Object(class_name.clone());
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
            classgen.gen_method(&method, &blocks, &cf.constant_pool)?;
        }

        if main {
            classgen.gen_main()?;
        }

        Ok(classgen.finish())
    }
}
