use failure::Fallible;

use classfile::attrs::Code;
use classfile::descriptors::ParameterDescriptor;

use classes::ClassGraph;
use frame::StackAndLocals;
use generate::CodeGen;
use loader::Class;
use translate::{self, VarIdGen};
use types::Type;

pub(crate) struct Compiler {
    classes: ClassGraph,
    codegen: CodeGen,
}

impl Compiler {
    pub fn new(classes: ClassGraph, codegen: CodeGen) -> Self {
        Self { classes, codegen }
    }

    pub fn compile(&mut self, class_name: &str) -> Fallible<()> {
        let cf = match self.classes.get(&class_name).unwrap() {
            Class::File(class_file) => class_file,
            class => bail!("unexpected class type {:?}", class),
        };

        let mut classgen = self.codegen.generate_class(&cf)?;

        classgen.gen_prelude()?;
        for method in cf.methods.iter() {
            let mut var_id_gen = VarIdGen::new();
            let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
            let mut args = Vec::new();
            if name == "<init>" {
                let arg_type = Type::UninitializedThis;
                args.push(var_id_gen.gen(arg_type));
            } else {
                let arg_type = Type::Object(class_name.to_owned());
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
            classgen.gen_method(&method, &blocks, &cf.constant_pool, &mut var_id_gen)?;
        }
        classgen.gen_main()?;
        Ok(())
    }
}
