use std::fmt::Write;
use std::sync::Arc;

use classfile::attrs::SourceFile;
use classfile::constant_pool::Constant;
use classfile::ClassFile;
use failure::Fallible;

use frontend::classes::ClassGraph;

use crate::codegen::common::*;
use crate::codegen::decls::DeclDatabase;
use crate::codegen::Target;
use crate::layout::{FieldLayoutMap, VTableMap};

pub struct PreludeCodeGen<'a> {
    pub out: &'a mut String,
    pub decls: &'a mut DeclDatabase,
    pub class: &'a Arc<ClassFile>,
    pub classes: &'a ClassGraph,
    pub vtables: &'a VTableMap,
    pub field_layouts: &'a FieldLayoutMap,
    pub var_id_gen: &'a mut TmpVarIdGen,
    pub target: &'a Arc<Target>,
}

impl<'a> PreludeCodeGen<'a> {
    pub fn gen_prelude(&mut self) -> Fallible<()> {
        let filename = self.class.attributes.get::<SourceFile>()?;

        writeln!(self.out, "; ModuleID = '{}'", self.class.get_name())?;
        writeln!(self.out, "source_filename = \"{}\"", filename.as_str())?;
        writeln!(
            self.out,
            "target datalayout = \"{}\"",
            self.target.data_layout
        )?;
        writeln!(self.out, "target triple = \"{}\"", self.target.triple)?;
        writeln!(self.out)?;

        writeln!(self.out, "%ref = type {{ i8*, i8* }}")?;

        writeln!(
            self.out,
            "declare i32 @_Jrt_start(i32, i8**, void (%ref) *)"
        )?;
        writeln!(self.out, "declare %ref @_Jrt_object_new(i64, i8*)")?;
        writeln!(self.out, "declare i8* @_Jrt_object_field_ptr(%ref)")?;
        writeln!(
            self.out,
            "declare i8* @_Jrt_object_vtable_lookup(%ref, i64)"
        )?;
        writeln!(
            self.out,
            "declare i8* @_Jrt_object_itable_lookup(%ref, i8*, i64)"
        )?;
        writeln!(self.out, "declare void @_Jrt_object_monitorenter(%ref)")?;
        writeln!(self.out, "declare void @_Jrt_object_monitorexit(%ref)")?;
        writeln!(self.out, "declare %ref @_Jrt_array_new(i32, i64)")?;
        writeln!(self.out, "declare i32 @_Jrt_array_length(%ref)")?;
        writeln!(self.out, "declare i8* @_Jrt_array_element_ptr(%ref)")?;
        writeln!(self.out, "declare void @_Jrt_throw(%ref) noreturn")?;
        writeln!(self.out, "declare void @_Jrt_abstract() noreturn")?;
        writeln!(self.out, "declare %ref @_Jrt_ldstr(i8*)")?;

        for index in self.class.constant_pool.indices() {
            if let Constant::String(string_const) =
                self.class.constant_pool.get_info(index).unwrap()
            {
                let utf8_index = string_const.string_index;
                writeln!(self.out)?;
                let utf8 = self.class.constant_pool.get_utf8(utf8_index).unwrap();
                writeln!(
                    self.out,
                    "@.str{} = internal constant [{} x i8] {}",
                    utf8_index.into_u16(),
                    utf8.len() + 1,
                    GenStringConst(&*utf8)
                )?;
            }
        }
        Ok(())
    }
}
