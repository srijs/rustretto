use classfile::attrs::Code;
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
        let class_file = match self.classes.get(&class_name)? {
            Class::File(class_file) => class_file,
            class => bail!("unexpected class type {:?}", class),
        };

        let mut classgen = self.codegen.generate_class(class_name)?;

        classgen.gen_vtable_const(&class_file)?;

        for method in class_file.methods.iter() {
            let name = class_file
                .constant_pool
                .get_utf8(method.name_index)
                .unwrap();
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
                classgen.gen_native_method(&method, &args, &class_file.constant_pool)?;
                continue;
            }

            if method.is_abstract() {
                classgen.gen_abstract_method(&method, &args, &class_file.constant_pool)?;
                continue;
            }

            let code = method.attributes.get::<Code>().unwrap();
            let state = StackAndLocals::new(code.max_stack, code.max_locals, &args);
            let blocks = translate::translate_method(
                code.disassemble(),
                state,
                &class_file.constant_pool,
                &mut var_id_gen,
            )?;
            classgen.gen_method(&method, &blocks, &class_file.constant_pool)?;

            if &**name == "<clinit>" {
                classgen.gen_class_init()?;
            }
        }

        if main {
            classgen.gen_main()?;
        }

        Ok(classgen.finish()?)
    }
}
