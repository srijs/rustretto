use std::fmt::Write;
use std::sync::Arc;

use classfile::{ClassFile, ConstantIndex, ConstantPool, FieldRef};
use failure::{bail, Fallible};
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::translate::{
    BinaryExpr, BinaryOperation, ConvertExpr, ConvertOperation, Expr, InvokeExpr, InvokeTarget, Op,
};
use frontend::types::Type;

use crate::codegen::common::*;
use crate::codegen::decls::DeclDatabase;
use crate::codegen::Target;
use crate::layout::{FieldLayoutMap, VTableMap};
use crate::mangle;

pub struct ExprCodeGen<'a> {
    pub out: &'a mut String,
    pub decls: &'a mut DeclDatabase,
    pub class: &'a Arc<ClassFile>,
    pub classes: &'a ClassGraph,
    pub vtables: &'a VTableMap,
    pub field_layouts: &'a FieldLayoutMap,
    pub var_id_gen: &'a mut TmpVarIdGen,
    pub target: &'a Arc<Target>,
}

impl<'a> ExprCodeGen<'a> {
    pub fn gen_expr(&mut self, expr: &Expr, consts: &ConstantPool, dest: Dest) -> Fallible<()> {
        match expr {
            Expr::String(index) => self.gen_load_string(*index, consts, dest)?,
            Expr::GetStatic(index) => self.gen_expr_get_static(*index, consts, dest)?,
            Expr::GetField(obj, index) => self.gen_expr_get_field(obj, *index, consts, dest)?,
            Expr::PutField(obj, index, value) => {
                self.gen_expr_put_field(obj, *index, value, consts)?
            }
            Expr::Invoke(subexpr) => self.gen_expr_invoke(subexpr, consts, dest)?,
            Expr::Binary(binary_expr) => self.gen_expr_binary(binary_expr, dest)?,
            Expr::LCmp(var1, var2) => self.gen_cmp_long(var1, var2, dest)?,
            Expr::New(class_name) => self.gen_expr_new(class_name, dest)?,
            Expr::ArrayNew(ctyp, count) => self.gen_expr_array_new(ctyp, count, dest)?,
            Expr::ArrayLength(aref) => self.gen_expr_array_length(aref, dest)?,
            Expr::ArrayLoad(ctyp, aref, idx) => self.gen_expr_array_load(ctyp, aref, idx, dest)?,
            Expr::ArrayStore(ctyp, aref, idx, val) => {
                self.gen_expr_array_store(ctyp, aref, idx, val)?
            }
            Expr::Convert(conv_expr) => self.gen_expr_convert(conv_expr, dest)?,
        }
        Ok(())
    }

    fn gen_expr_new(&mut self, class_name: &StrBuf, dest: Dest) -> Fallible<()> {
        let object_type = self.decls.add_object_type(class_name)?;
        let vtable_type = self.decls.add_vtable_type(class_name)?;

        if let Dest::Assign(assign) = dest {
            let tmp_size_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = getelementptr {otyp}, {otyp}* null, i32 1",
                tmp_size_ptr,
                otyp = object_type
            )?;
            let tmp_size_int = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = ptrtoint {otyp}* %t{} to i64",
                tmp_size_int,
                tmp_size_ptr,
                otyp = object_type
            )?;
            writeln!(
                self.out,
                "  {} = call %ref @_Jrt_new(i64 %t{}, i8* bitcast ({vtyp}* @{vtable} to i8*))",
                assign,
                tmp_size_int,
                vtyp = vtable_type,
                vtable = mangle::mangle_vtable_name(class_name)
            )?;
        }
        Ok(())
    }

    fn gen_load_string(
        &mut self,
        index: ConstantIndex,
        consts: &ConstantPool,
        dest: Dest,
    ) -> Fallible<()> {
        let len = consts.get_utf8(index).unwrap().len();
        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = call %ref @_Jrt_ldstr(i32 {}, i8* getelementptr ([{} x i8], [{} x i8]* @.str{}, i64 0, i64 0))",
                assign,
                len,
                len + 1,
                len + 1,
                index.into_u16()
            )?;
        }
        Ok(())
    }

    fn gen_expr_invoke(
        &mut self,
        expr: &InvokeExpr,
        consts: &ConstantPool,
        dest: Dest,
    ) -> Fallible<()> {
        let method_ref = consts.get_method_ref(expr.index).unwrap();
        let method_name = consts.get_utf8(method_ref.name_index).unwrap();
        let method_class = consts.get_class(method_ref.class_index).unwrap();
        let method_class_name = consts.get_utf8(method_class.name_index).unwrap();

        let fptr = match expr.target {
            InvokeTarget::Virtual(ref var) => {
                let vtable_type = self.decls.add_vtable_type(method_class_name)?;

                let vtable = self.vtables.get(method_class_name)?;
                let (offset, _) = vtable.get(method_name, &method_ref.descriptor).unwrap();

                writeln!(self.out, "  ; prepare virtual dispatch")?;
                let tmp_vtblraw = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{vtblraw} = extractvalue %ref {}, 1",
                    OpVal(var),
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_vtbl = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{vtbl} = bitcast i8* %t{vtblraw} to {vtyp}*",
                    vtyp = vtable_type,
                    vtbl = tmp_vtbl,
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_fptrptr = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptrptr} = getelementptr {vtyp}, {vtyp}* %t{vtbl}, i64 0, i32 {offset}",
                    offset = offset + 1,
                    vtyp = vtable_type,
                    vtbl = tmp_vtbl,
                    fptrptr = tmp_fptrptr
                )?;
                let tmp_fptr = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptr} = load {ftyp}*, {ftyp}** %t{fptrptr}",
                    fptr = tmp_fptr,
                    fptrptr = tmp_fptrptr,
                    ftyp = tlt_function_type(&method_ref.descriptor)
                )?;
                writeln!(self.out, "  ; invoke {}", method_name)?;

                format!("%t{}", tmp_fptr)
            }
            _ => format!(
                "@{}",
                mangle::mangle_method_name(
                    method_class_name,
                    method_name,
                    &method_ref.descriptor.ret,
                    &method_ref.descriptor.params
                )
            ),
        };

        if let Dest::Assign(assign) = dest {
            write!(self.out, "  {} = ", assign)?;
        } else {
            write!(self.out, "  ")?;
        }

        write!(
            self.out,
            "call {return_type} {fptr}(",
            fptr = fptr,
            return_type = tlt_return_type(&method_ref.descriptor.ret)
        )?;

        let mut args = vec![];

        match expr.target {
            InvokeTarget::Static => {}
            InvokeTarget::Special(ref var) => args.push(format!("%ref {}", OpVal(var))),
            InvokeTarget::Virtual(ref var) => args.push(format!("%ref {}", OpVal(var))),
        };

        for var in expr.args.iter() {
            args.push(format!("{} {}", tlt_type(&var.get_type()), OpVal(&var)));
        }

        for (idx, arg) in args.iter().enumerate() {
            if idx > 0 {
                write!(self.out, ", {}", arg)?;
            } else {
                write!(self.out, "{}", arg)?;
            }
        }

        writeln!(self.out, ")")?;
        Ok(())
    }

    fn gen_expr_binary(&mut self, binary_expr: &BinaryExpr, dest: Dest) -> Fallible<()> {
        match binary_expr.operation {
            BinaryOperation::Add => self.gen_expr_binary_simple("add", binary_expr, dest)?,
            BinaryOperation::Sub => self.gen_expr_binary_simple("sub", binary_expr, dest)?,
            BinaryOperation::BitwiseAnd => self.gen_expr_binary_simple("and", binary_expr, dest)?,
            BinaryOperation::BitwiseOr => self.gen_expr_binary_simple("or", binary_expr, dest)?,
            BinaryOperation::ShiftLeft => {
                if let Dest::Assign(assign) = dest {
                    let tmp_masked = self.var_id_gen.gen();
                    writeln!(
                        self.out,
                        "  %t{} = and {} {}, 31",
                        tmp_masked,
                        tlt_type(&binary_expr.operand_right.get_type()),
                        OpVal(&binary_expr.operand_right)
                    )?;
                    writeln!(
                        self.out,
                        "  {} = shl {} {}, %t{}",
                        assign,
                        tlt_type(&binary_expr.result_type),
                        OpVal(&binary_expr.operand_left),
                        tmp_masked
                    )?;
                }
            }
        }
        Ok(())
    }

    fn gen_expr_binary_simple(
        &mut self,
        operation: &str,
        binary_expr: &BinaryExpr,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = {} {} {}, {}",
                assign,
                operation,
                tlt_type(&binary_expr.result_type),
                OpVal(&binary_expr.operand_left),
                OpVal(&binary_expr.operand_right)
            )?;
        }
        Ok(())
    }

    fn gen_expr_convert(&mut self, conv_expr: &ConvertExpr, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            match conv_expr.operation {
                ConvertOperation::IntToChar => {
                    let tmp_trunc = self.var_id_gen.gen();
                    writeln!(
                        self.out,
                        "  %t{} = trunc i32 {} to i8",
                        tmp_trunc,
                        OpVal(&conv_expr.operand)
                    )?;
                    writeln!(self.out, "  {} = zext i8 %t{} to i32", assign, tmp_trunc)?;
                }
            }
        }
        Ok(())
    }

    fn gen_expr_array_new(&mut self, ctyp: &Type, count: &Op, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let component_type = tlt_array_component_type(ctyp);

            let tmp_component_size_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = getelementptr {ctyp}, {ctyp}* null, i64 1",
                tmp_component_size_ptr,
                ctyp = component_type
            )?;
            let tmp_component_size_int = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = ptrtoint {ctyp}* %t{} to i64",
                tmp_component_size_int,
                tmp_component_size_ptr,
                ctyp = component_type
            )?;
            let tmp_count_wide = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = zext i32 {} to i64",
                tmp_count_wide,
                OpVal(count)
            )?;
            let tmp_components_size_int = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = mul i64 %t{}, %t{}",
                tmp_components_size_int, tmp_component_size_int, tmp_count_wide
            )?;
            let tmp_total_size_int = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = add i64 %t{}, 64",
                tmp_total_size_int, tmp_components_size_int
            )?;
            writeln!(
                self.out,
                "  {} = call %ref @_Jrt_new(i64 %t{}, i8* bitcast (%{vtable}* @{vtable} to i8*))",
                assign,
                tmp_total_size_int,
                vtable = mangle::mangle_vtable_name("java/lang/Object")
            )?;
            if let DestAssign::Var(assign_op) = assign {
                let tmp_length_ptr = self.var_id_gen.gen();
                self.gen_get_array_length_ptr(
                    &Op::Var(assign_op),
                    Dest::Assign(DestAssign::Tmp(tmp_length_ptr)),
                )?;
                writeln!(
                    self.out,
                    "store i32 {}, i32* %t{}",
                    OpVal(count),
                    tmp_length_ptr
                )?;
            } else {
                bail!("can't assign array to tmp dest");
            }
        }
        Ok(())
    }

    fn gen_expr_array_length(&mut self, aref: &Op, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let tmp_length_ptr = self.var_id_gen.gen();
            self.gen_get_array_length_ptr(aref, Dest::Assign(DestAssign::Tmp(tmp_length_ptr)))?;

            writeln!(
                self.out,
                "  {} = load i32, i32* %t{}",
                assign, tmp_length_ptr
            )?;
        }
        Ok(())
    }

    fn gen_expr_array_load(
        &mut self,
        ctyp: &Type,
        aref: &Op,
        idx: &Op,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let tmp_array_ptr = self.var_id_gen.gen();
            let component_type = self.gen_get_array_ptr(
                ctyp,
                aref,
                idx,
                Dest::Assign(DestAssign::Tmp(tmp_array_ptr)),
            )?;

            writeln!(
                self.out,
                "  {} = load {ctyp}, {ctyp}* %t{}",
                assign,
                tmp_array_ptr,
                ctyp = component_type
            )?;
        }
        Ok(())
    }

    fn gen_expr_array_store(
        &mut self,
        ctyp: &Type,
        aref: &Op,
        idx: &Op,
        value: &Op,
    ) -> Fallible<()> {
        let tmp_array_ptr = self.var_id_gen.gen();
        let component_type = self.gen_get_array_ptr(
            ctyp,
            aref,
            idx,
            Dest::Assign(DestAssign::Tmp(tmp_array_ptr)),
        )?;

        writeln!(
            self.out,
            "  store {ctyp} {}, {ctyp}* %t{}",
            OpVal(value),
            tmp_array_ptr,
            ctyp = component_type
        )?;
        Ok(())
    }

    fn gen_expr_get_static(
        &mut self,
        index: ConstantIndex,
        consts: &ConstantPool,
        dest: Dest,
    ) -> Fallible<()> {
        let field_ref = consts.get_field_ref(index).unwrap();
        let field_name = consts.get_utf8(field_ref.name_index).unwrap();
        let field_class = consts.get_class(field_ref.class_index).unwrap();
        let field_class_name = consts.get_utf8(field_class.name_index).unwrap();
        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = load {field_type}, {field_type}* @{field_name}",
                assign,
                field_type = tlt_field_type(&field_ref.descriptor),
                field_name = mangle::mangle_field_name(field_class_name, field_name)
            )?;
        }
        Ok(())
    }

    fn gen_expr_get_field(
        &mut self,
        object: &Op,
        index: ConstantIndex,
        consts: &ConstantPool,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let tmp_field_ptr = self.var_id_gen.gen();
            let field_ref = self.gen_get_field_ptr(
                object,
                index,
                consts,
                Dest::Assign(DestAssign::Tmp(tmp_field_ptr)),
            )?;

            writeln!(
                self.out,
                "  {} = load {field_type}, {field_type}* %t{}",
                assign,
                tmp_field_ptr,
                field_type = tlt_field_type(&field_ref.descriptor)
            )?;
        }
        Ok(())
    }

    fn gen_expr_put_field(
        &mut self,
        object: &Op,
        index: ConstantIndex,
        value: &Op,
        consts: &ConstantPool,
    ) -> Fallible<()> {
        let tmp_field_ptr = self.var_id_gen.gen();
        let field_ref = self.gen_get_field_ptr(
            object,
            index,
            consts,
            Dest::Assign(DestAssign::Tmp(tmp_field_ptr)),
        )?;

        writeln!(
            self.out,
            "  store {field_type} {}, {field_type}* %t{}",
            OpVal(value),
            tmp_field_ptr,
            field_type = tlt_field_type(&field_ref.descriptor)
        )?;
        Ok(())
    }

    fn gen_get_array_length_ptr(&mut self, aref: &Op, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let tmp_object_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = extractvalue %ref {}, 0",
                tmp_object_ptr,
                OpVal(aref)
            )?;

            writeln!(
                self.out,
                "  {} = bitcast i8* %t{} to i32*",
                assign, tmp_object_ptr
            )?;
        }
        Ok(())
    }

    fn gen_get_array_ptr(
        &mut self,
        ctyp: &Type,
        aref: &Op,
        idx: &Op,
        dest: Dest,
    ) -> Fallible<&'static str> {
        let component_type = tlt_array_component_type(&ctyp);

        if let Dest::Assign(assign) = dest {
            let tmp_length_ptr = self.var_id_gen.gen();
            self.gen_get_array_length_ptr(aref, Dest::Assign(DestAssign::Tmp(tmp_length_ptr)))?;

            let tmp_member_start_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = getelementptr i32, i32* %t{}, i64 1",
                tmp_member_start_ptr, tmp_length_ptr
            )?;

            let tmp_member_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = bitcast i32* %t{} to {ctyp}*",
                tmp_member_ptr,
                tmp_member_start_ptr,
                ctyp = component_type
            )?;

            writeln!(
                self.out,
                "  {} = getelementptr {ctyp}, {ctyp}* %t{}, i32 {idx}",
                assign,
                tmp_member_ptr,
                idx = OpVal(idx),
                ctyp = component_type
            )?;
        }
        Ok(component_type)
    }

    fn gen_get_field_ptr(
        &mut self,
        object: &Op,
        index: ConstantIndex,
        consts: &ConstantPool,
        dest: Dest,
    ) -> Fallible<FieldRef> {
        let field_ref = consts.get_field_ref(index).unwrap();
        if let Dest::Assign(assign) = dest {
            let field_name = consts.get_utf8(field_ref.name_index).unwrap();
            let field_class = consts.get_class(field_ref.class_index).unwrap();
            let field_class_name = consts.get_utf8(field_class.name_index).unwrap();
            let field_layout = self.field_layouts.get(field_class_name)?;

            let object_type = self.decls.add_object_type(field_class_name)?;

            let tmp_object_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = extractvalue %ref {}, 0",
                tmp_object_ptr,
                OpVal(object)
            )?;

            let tmp_object_ptr_cast = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = bitcast i8* %t{} to {}*",
                tmp_object_ptr_cast, tmp_object_ptr, object_type
            )?;

            writeln!(
                self.out,
                "  {} = getelementptr {otyp}, {otyp}* %t{}, i64 0, i32 {field_index}",
                assign,
                tmp_object_ptr_cast,
                otyp = object_type,
                field_index = field_layout.get(field_name, &field_ref.descriptor).unwrap()
            )?;
        }
        Ok(field_ref)
    }

    fn gen_cmp_long(&mut self, var1: &Op, var2: &Op, dest: Dest) -> Fallible<()> {
        let tmp_lt = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{} = icmp slt i64 {}, {}",
            tmp_lt,
            OpVal(var1),
            OpVal(var2)
        )?;
        let tmp_lt_ext = self.var_id_gen.gen();
        writeln!(self.out, "  %t{} = zext i1 %t{} to i32", tmp_lt_ext, tmp_lt)?;
        let tmp_gt = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{} = icmp sgt i64 {}, {}",
            tmp_gt,
            OpVal(var1),
            OpVal(var2)
        )?;
        let tmp_gt_ext = self.var_id_gen.gen();
        writeln!(self.out, "  %t{} = zext i1 %t{} to i32", tmp_gt_ext, tmp_gt)?;
        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = sub i32 %t{}, %t{}",
                assign, tmp_gt_ext, tmp_lt_ext
            )?;
        }
        Ok(())
    }
}
