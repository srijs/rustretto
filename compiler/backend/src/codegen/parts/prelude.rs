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

        writeln!(self.out, "declare %ref @_Jrt_new(i64, i8*)")?;
        writeln!(self.out, "declare void @_Jrt_throw(%ref) noreturn")?;
        writeln!(self.out, "declare %ref @_Jrt_ldstr(i32, i8*)")?;

        for index in self.class.constant_pool.indices() {
            if let Constant::String(string_const) =
                self.class.constant_pool.get_info(index).unwrap()
            {
                let utf8_index = string_const.string_index;
                writeln!(self.out)?;
                let utf8 = self.class.constant_pool.get_utf8(utf8_index).unwrap();
                write!(
                    self.out,
                    "@.str{} = internal constant [{} x i8] [",
                    utf8_index.into_u16(),
                    utf8.len() + 1
                )?;
                for byte in utf8.as_bytes() {
                    write!(self.out, " i8 {},", byte)?;
                }
                writeln!(self.out, " i8 0 ]")?;
            }
        }
        Ok(())
    }
}
