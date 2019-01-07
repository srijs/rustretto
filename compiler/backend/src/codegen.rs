use std::fmt::Write;
use std::sync::Arc;

use classfile::attrs::SourceFile;
use classfile::descriptors::{
    ArrayType, FieldType, ObjectType, ParameterDescriptor, ReturnTypeDescriptor,
};
use classfile::{ClassFile, ConstantPool, Method};
use failure::{bail, Fallible};
use strbuf::StrBuf;

use frontend::blocks::BlockGraph;
use frontend::classes::ClassGraph;
use frontend::loader::Class;
use frontend::translate::VarId;

use crate::layout::{FieldLayoutMap, VTableMap};
use crate::mangle;

mod common;
mod decls;
mod parts;

use self::common::*;
use self::decls::DeclDatabase;
use self::parts::{MethodCodeGen, PreludeCodeGen};

pub struct Target {
    pub triple: String,
    pub data_layout: String,
}

pub struct CodeGen {
    classes: ClassGraph,
    vtables: VTableMap,
    field_layouts: FieldLayoutMap,
    target: Arc<Target>,
}

impl CodeGen {
    pub fn try_new(classes: ClassGraph, target: Target) -> Fallible<Self> {
        let vtables = VTableMap::new(classes.clone());
        let field_layouts = FieldLayoutMap::new(classes.clone());
        Ok(CodeGen {
            classes,
            vtables,
            field_layouts,
            target: Arc::new(target),
        })
    }

    pub fn generate_class(&self, name: &StrBuf) -> Fallible<ClassCodeGen> {
        let class = match self.classes.get(name)? {
            Class::File(class_file) => class_file,
            _ => bail!("can't generate code for array class"),
        };
        let _class_name = class
            .constant_pool
            .get_utf8(class.get_this_class().name_index)
            .unwrap();
        let _source_file = class.attributes.get::<SourceFile>()?;

        Ok(ClassCodeGen {
            out: String::new(),
            decls: DeclDatabase::new(&self.classes, &self.vtables, &self.field_layouts),
            class: class.clone(),
            classes: self.classes.clone(),
            vtables: self.vtables.clone(),
            field_layouts: self.field_layouts.clone(),
            var_id_gen: TmpVarIdGen::new(),
            target: self.target.clone(),
        })
    }
}

pub struct ClassCodeGen {
    out: String,
    decls: DeclDatabase,
    class: Arc<ClassFile>,
    classes: ClassGraph,
    vtables: VTableMap,
    field_layouts: FieldLayoutMap,
    var_id_gen: TmpVarIdGen,
    target: Arc<Target>,
}

impl ClassCodeGen {
    pub fn finish(mut self) -> Fallible<String> {
        let mut out = String::new();
        self.gen_prelude(&mut out)?;
        for entry in self.decls.entries() {
            writeln!(out, "{}", entry)?;
        }
        out.push_str(&self.out);
        Ok(out)
    }

    pub fn gen_main(&mut self) -> Fallible<()> {
        let class_name = self
            .class
            .constant_pool
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();
        writeln!(self.out, "define i32 @main() {{")?;
        writeln!(
            self.out,
            "  call void @{}(%ref zeroinitializer)",
            mangle::mangle_method_name(
                class_name,
                "main",
                &ReturnTypeDescriptor::Void,
                &[ParameterDescriptor::Field(FieldType::Array(ArrayType {
                    component_type: Box::new(FieldType::Object(ObjectType {
                        class_name: "java.lang.String".to_owned()
                    }))
                }))]
            )
        )?;
        writeln!(self.out, "  ret i32 0")?;
        writeln!(self.out, "}}")?;
        Ok(())
    }

    pub fn gen_vtable_const(&mut self, class_name: &StrBuf) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;
        let vtable_name = mangle::mangle_vtable_name(class_name);

        let vtable_type = self.decls.add_vtable_type(class_name)?;

        writeln!(
            self.out,
            "@{vtable} = constant {vtyp} {{",
            vtable = vtable_name,
            vtyp = vtable_type
        )?;
        write!(self.out, "  i32 {}", vtable.len())?;
        if !vtable.is_empty() {
            writeln!(self.out, ",")?;
        }
        for (idx, (key, target)) in vtable.iter().enumerate() {
            if target.class_name != *self.class.get_name() {
                self.decls.add_instance_method(
                    &target.class_name,
                    &key.method_name,
                    &key.method_descriptor,
                )?;
            }

            write!(
                self.out,
                "  {} * @{}",
                tlt_function_type(&key.method_descriptor),
                mangle::mangle_method_name(
                    &target.class_name,
                    &key.method_name,
                    &key.method_descriptor.ret,
                    &key.method_descriptor.params
                )
            )?;
            if idx < vtable.len() - 1 {
                writeln!(self.out, ",")?;
            } else {
                writeln!(self.out)?;
            }
        }
        writeln!(self.out, "}}")?;

        Ok(())
    }

    fn gen_prelude(&mut self, out: &mut String) -> Fallible<()> {
        let mut prelude_code_gen = PreludeCodeGen {
            out,
            decls: &mut self.decls,
            class: &self.class,
            classes: &self.classes,
            vtables: &self.vtables,
            field_layouts: &self.field_layouts,
            var_id_gen: &mut self.var_id_gen,
            target: &self.target,
        };
        prelude_code_gen.gen_prelude()
    }

    pub fn gen_method(
        &mut self,
        method: &Method,
        blocks: &BlockGraph,
        consts: &ConstantPool,
    ) -> Fallible<()> {
        let mut method_code_gen = MethodCodeGen {
            out: &mut self.out,
            decls: &mut self.decls,
            class: &self.class,
            classes: &self.classes,
            vtables: &self.vtables,
            field_layouts: &self.field_layouts,
            var_id_gen: &mut self.var_id_gen,
            target: &self.target,
        };
        method_code_gen.gen_method(method, blocks, consts)
    }

    pub fn gen_native_method(
        &mut self,
        method: &Method,
        args: &[VarId],
        consts: &ConstantPool,
    ) -> Fallible<()> {
        let class_name = consts
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();
        let method_name = consts.get_utf8(method.name_index).unwrap();
        write!(
            self.out,
            "\ndeclare {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method.descriptor.ret),
            mangled_name = mangle::mangle_method_name(
                class_name,
                method_name,
                &method.descriptor.ret,
                &method.descriptor.params
            )
        )?;
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ")?;
            }
            write!(self.out, "{}", tlt_type(&arg.0))?;
        }
        writeln!(self.out, ")")?;
        Ok(())
    }

    pub fn gen_class_init(&mut self) -> Fallible<()> {
        let mangled_name = mangle::mangle_method_name(
            self.class.get_name(),
            "<clinit>",
            &ReturnTypeDescriptor::Void,
            &[],
        );
        writeln!(
            self.out,
            "@llvm.global_ctors = appending global [1 x {{ i32, void ()*, i8* }}] ["
        )?;
        writeln!(self.out, "  {{ i32, void ()*, i8* }}")?;
        writeln!(
            self.out,
            "  {{ i32 65535, void ()* @{}, i8* null }}",
            mangled_name
        )?;
        writeln!(self.out, "]")?;
        Ok(())
    }
}
