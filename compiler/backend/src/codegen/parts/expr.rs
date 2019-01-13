use std::fmt::Write;
use std::sync::Arc;

use classfile::{ClassFile, ConstantIndex, ConstantPool, FieldRef};
use failure::Fallible;
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::translate::{
    AComparator, BinaryExpr, BinaryOperation, CompareExpr, Const, ConvertExpr, ConvertOperation,
    Expr, IComparator, InvokeExpr, InvokeTarget, MonitorStateTransition, NaNCmpMode, Op,
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
            Expr::Compare(compare_expr) => self.gen_expr_compare(compare_expr, dest)?,
            Expr::New(class_name) => self.gen_expr_new(class_name, dest)?,
            Expr::ArrayNew(ctyp, count) => self.gen_expr_array_new(ctyp, count, dest)?,
            Expr::ArrayLength(aref) => self.gen_expr_array_length(aref, dest)?,
            Expr::ArrayLoad(ctyp, aref, idx) => self.gen_expr_array_load(ctyp, aref, idx, dest)?,
            Expr::ArrayStore(ctyp, aref, idx, val) => {
                self.gen_expr_array_store(ctyp, aref, idx, val)?
            }
            Expr::Convert(conv_expr) => self.gen_expr_convert(conv_expr, dest)?,
            Expr::Monitor(oref, transition) => self.gen_expr_monitor(oref, transition)?,
        }
        Ok(())
    }

    fn gen_expr_new(&mut self, class_name: &StrBuf, dest: Dest) -> Fallible<()> {
        let object_type = self.decls.add_object_type(class_name)?;
        let vtable_type = self.decls.add_vtable_type(class_name)?;
        let vtable_const = self.decls.add_vtable_const(class_name)?;

        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = call %ref @_Jrt_object_new(i64 {size}, i8* bitcast ({vtyp}* {vtbl} to i8*))",
                assign,
                size = GenSizeOf(&object_type),
                vtyp = vtable_type,
                vtbl = vtable_const
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
                "  {} = call %ref @_Jrt_ldstr(i8* getelementptr ([{} x i8], [{} x i8]* @.str{}, i64 0, i64 0))",
                assign,
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
        let method_name = consts.get_utf8(expr.method.name_index).unwrap();
        let method_class = consts.get_class(expr.method.class_index).unwrap();
        let method_class_name = consts.get_utf8(method_class.name_index).unwrap();

        let fptr = match expr.target {
            InvokeTarget::Virtual(ref var) => {
                let vtable = self.vtables.get(method_class_name)?;
                let target = vtable.get(method_name, &expr.method.descriptor).unwrap();

                let tmp_fptr = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptr} = call i8* @_Jrt_object_vtable_lookup(%ref {object}, i64 {index})",
                    fptr = tmp_fptr,
                    object = OpVal(var),
                    index = target.method_index_lower
                )?;
                let tmp_fptr_cast = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptr_cast} = bitcast i8* %t{fptr} to {ftyp}*",
                    fptr_cast = tmp_fptr_cast,
                    fptr = tmp_fptr,
                    ftyp = GenFunctionType(&expr.method.descriptor)
                )?;

                format!("%t{}", tmp_fptr_cast)
            }
            InvokeTarget::Interface(ref var) => {
                let vtable = self.vtables.get(method_class_name)?;
                let target = vtable.get(method_name, &expr.method.descriptor).unwrap();
                let iface_vtable_type = self.decls.add_vtable_type(method_class_name)?;
                let iface_vtable_const = self.decls.add_vtable_const(method_class_name)?;

                let tmp_fptr = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptr} = call i8* @_Jrt_object_itable_lookup(%ref {object}, i8* bitcast ({ivtyp}* {ivtbl} to i8*), i64 {index})",
                    fptr = tmp_fptr,
                    object = OpVal(var),
                    ivtyp = iface_vtable_type,
                    ivtbl = iface_vtable_const,
                    index = target.method_index_lower
                )?;
                let tmp_fptr_cast = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptr_cast} = bitcast i8* %t{fptr} to {ftyp}*",
                    fptr_cast = tmp_fptr_cast,
                    fptr = tmp_fptr,
                    ftyp = GenFunctionType(&expr.method.descriptor)
                )?;

                format!("%t{}", tmp_fptr_cast)
            }
            InvokeTarget::Special(_) => {
                if method_class_name != self.class.get_name() {
                    self.decls.add_instance_method(
                        method_class_name,
                        method_name,
                        &expr.method.descriptor,
                    )?;
                }
                format!(
                    "@{}",
                    mangle::mangle_method_name(
                        method_class_name,
                        method_name,
                        &expr.method.descriptor.ret,
                        &expr.method.descriptor.params
                    )
                )
            }
            InvokeTarget::Static => {
                if method_class_name != self.class.get_name() {
                    self.decls.add_static_method(
                        method_class_name,
                        method_name,
                        &expr.method.descriptor,
                    )?;
                }
                format!(
                    "@{}",
                    mangle::mangle_method_name(
                        method_class_name,
                        method_name,
                        &expr.method.descriptor.ret,
                        &expr.method.descriptor.params
                    )
                )
            }
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
            return_type = tlt_return_type(&expr.method.descriptor.ret)
        )?;

        let mut args = vec![];

        match expr.target {
            InvokeTarget::Static => {}
            InvokeTarget::Special(ref var) => args.push(format!("%ref {}", OpVal(var))),
            InvokeTarget::Virtual(ref var) => args.push(format!("%ref {}", OpVal(var))),
            InvokeTarget::Interface(ref var) => args.push(format!("%ref {}", OpVal(var))),
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

    fn gen_expr_monitor(&mut self, op: &Op, transition: &MonitorStateTransition) -> Fallible<()> {
        match transition {
            MonitorStateTransition::Enter => {
                writeln!(
                    self.out,
                    "  call void @_Jrt_object_monitorenter(%ref {})",
                    OpVal(op)
                )?;
            }
            MonitorStateTransition::Exit => {
                writeln!(
                    self.out,
                    "  call void @_Jrt_object_monitorexit(%ref {})",
                    OpVal(op)
                )?;
            }
        }
        Ok(())
    }

    fn gen_expr_binary(&mut self, binary_expr: &BinaryExpr, dest: Dest) -> Fallible<()> {
        match binary_expr.operation {
            BinaryOperation::Add => self.gen_expr_binary_simple("add", binary_expr, dest)?,
            BinaryOperation::Sub => self.gen_expr_binary_simple("sub", binary_expr, dest)?,
            BinaryOperation::BitwiseAnd => self.gen_expr_binary_simple("and", binary_expr, dest)?,
            BinaryOperation::BitwiseOr => self.gen_expr_binary_simple("or", binary_expr, dest)?,
            BinaryOperation::BitwiseXor => self.gen_expr_binary_simple("xor", binary_expr, dest)?,
            BinaryOperation::ShiftLeft => self.gen_expr_binary_shift("shl", binary_expr, dest)?,
            BinaryOperation::ShiftRightLogical => {
                self.gen_expr_binary_shift("lshr", binary_expr, dest)?
            }
            BinaryOperation::ShiftRightArithmetic => {
                self.gen_expr_binary_shift("ashr", binary_expr, dest)?
            }
        }
        Ok(())
    }

    fn gen_expr_binary_shift(
        &mut self,
        operation: &str,
        binary_expr: &BinaryExpr,
        dest: Dest,
    ) -> Fallible<()> {
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
                "  {} = {} {} {}, %t{}",
                assign,
                operation,
                tlt_type(&binary_expr.result_type),
                OpVal(&binary_expr.operand_left),
                tmp_masked
            )?;
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

    fn gen_expr_compare(&mut self, expr: &CompareExpr, dest: Dest) -> Fallible<()> {
        match expr {
            CompareExpr::ICmp(comp, var1, var2) => {
                self.gen_expr_compare_int(comp, var1, var2, dest)
            }
            CompareExpr::ACmp(comp, var1, var2) => {
                self.gen_expr_compare_addr(comp, var1, var2, dest)
            }
            CompareExpr::LCmp(var1, var2) => self.gen_expr_compare_long(var1, var2, dest),
            CompareExpr::FCmp(var1, var2, mode) => self.gen_expr_compare_fp(var1, var2, mode, dest),
            CompareExpr::DCmp(var1, var2, mode) => self.gen_expr_compare_fp(var1, var2, mode, dest),
        }
    }

    fn gen_expr_convert(&mut self, conv_expr: &ConvertExpr, dest: Dest) -> Fallible<()> {
        match conv_expr.operation {
            ConvertOperation::IntToChar => {
                self.gen_expr_convert_truncate_and_extend(&conv_expr.operand, "i8", false, dest)
            }
            ConvertOperation::IntToByte => {
                self.gen_expr_convert_truncate_and_extend(&conv_expr.operand, "i8", true, dest)
            }
            ConvertOperation::IntToShort => {
                self.gen_expr_convert_truncate_and_extend(&conv_expr.operand, "i16", true, dest)
            }
        }
    }

    fn gen_expr_convert_truncate_and_extend(
        &mut self,
        op: &Op,
        via: &str,
        sign: bool,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let op_type = tlt_type(&op.get_type());
            let tmp_trunc = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = trunc {typ} {op} to {via}",
                tmp_trunc,
                typ = op_type,
                op = OpVal(op),
                via = via
            )?;
            if sign {
                writeln!(
                    self.out,
                    "  {} = sext {via} %t{} to {typ}",
                    assign,
                    tmp_trunc,
                    typ = op_type,
                    via = via
                )?;
            } else {
                writeln!(
                    self.out,
                    "  {} = zext {via} %t{} to {typ}",
                    assign,
                    tmp_trunc,
                    typ = op_type,
                    via = via
                )?;
            }
        }
        Ok(())
    }

    fn gen_expr_array_new(&mut self, ctyp: &Type, count: &Op, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let component_type = tlt_array_component_type(ctyp);
            writeln!(
                self.out,
                "  {} = call %ref @_Jrt_array_new(i32 {count}, i64 {width})",
                assign,
                count = OpVal(count),
                width = GenSizeOf(&component_type)
            )?;
        }
        Ok(())
    }

    fn gen_expr_array_length(&mut self, aref: &Op, dest: Dest) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = call i32 @_Jrt_array_length(%ref {})",
                assign,
                OpVal(aref)
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

            let tmp_extend = self.var_id_gen.gen();
            let load_assign = match ctyp {
                Type::Boolean | Type::Byte | Type::Short | Type::Char => {
                    DestAssign::Tmp(tmp_extend)
                }
                _ => assign.clone(),
            };

            writeln!(
                self.out,
                "  {} = load {ctyp}, {ctyp}* %t{}",
                load_assign,
                tmp_array_ptr,
                ctyp = component_type
            )?;

            match ctyp {
                Type::Boolean | Type::Byte | Type::Short => {
                    writeln!(
                        self.out,
                        "   {} = sext {ctyp} %t{} to {vtyp}",
                        assign,
                        tmp_extend,
                        vtyp = tlt_type(&ctyp),
                        ctyp = component_type
                    )?;
                }
                Type::Char => {
                    writeln!(
                        self.out,
                        "   {} = zext {ctyp} %t{} to {vtyp}",
                        assign,
                        tmp_extend,
                        vtyp = tlt_type(&ctyp),
                        ctyp = component_type
                    )?;
                }
                _ => {}
            }
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

        let truncated = match ctyp {
            Type::Boolean | Type::Byte | Type::Short | Type::Char => {
                let tmp_trunc = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "   %t{} = trunc {vtyp} {val} to {ctyp}",
                    tmp_trunc,
                    vtyp = tlt_type(&value.get_type()),
                    val = OpVal(value),
                    ctyp = component_type
                )?;
                format!("%t{}", tmp_trunc)
            }
            _ => OpVal(value).to_string(),
        };

        writeln!(
            self.out,
            "  store {ctyp} {val}, {ctyp}* %t{aptr}",
            ctyp = component_type,
            val = truncated,
            aptr = tmp_array_ptr,
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

        let field_identifier =
            self.decls
                .add_static_field(field_class_name, field_name, &field_ref.descriptor)?;

        if let Dest::Assign(assign) = dest {
            writeln!(
                self.out,
                "  {} = load {ftyp}, {ftyp}* {field}",
                assign,
                ftyp = tlt_field_type(&field_ref.descriptor),
                field = field_identifier
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

    fn gen_get_array_ptr(
        &mut self,
        ctyp: &Type,
        aref: &Op,
        idx: &Op,
        dest: Dest,
    ) -> Fallible<&'static str> {
        let component_type = tlt_array_component_type(&ctyp);

        if let Dest::Assign(assign) = dest {
            let tmp_element_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = call i8* @_Jrt_array_element_ptr(%ref {})",
                tmp_element_ptr,
                OpVal(aref)
            )?;

            let tmp_element_ptr_cast = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = bitcast i8* %t{} to {ctyp}*",
                tmp_element_ptr_cast,
                tmp_element_ptr,
                ctyp = component_type
            )?;

            writeln!(
                self.out,
                "  {} = getelementptr {ctyp}, {ctyp}* %t{}, i32 {idx}",
                assign,
                tmp_element_ptr_cast,
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

            let tmp_field_ptr = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = call i8* @_Jrt_object_field_ptr(%ref {})",
                tmp_field_ptr,
                OpVal(object)
            )?;

            let tmp_field_ptr_cast = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = bitcast i8* %t{} to {}*",
                tmp_field_ptr_cast, tmp_field_ptr, object_type
            )?;

            writeln!(
                self.out,
                "  {} = getelementptr {otyp}, {otyp}* %t{}, i64 0, i32 {field_index}",
                assign,
                tmp_field_ptr_cast,
                otyp = object_type,
                field_index = field_layout.get(field_name, &field_ref.descriptor).unwrap()
            )?;
        }
        Ok(field_ref)
    }

    fn gen_expr_compare_int(
        &mut self,
        comp: &IComparator,
        var1: &Op,
        var2: &Op,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let code = match comp {
                IComparator::Lt => "slt",
                IComparator::Le => "sle",
                IComparator::Eq => "eq",
                IComparator::Ne => "ne",
                IComparator::Ge => "sge",
                IComparator::Gt => "sgt",
            };
            let tmp_i1 = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = icmp {} i32 {}, {}",
                tmp_i1,
                code,
                OpVal(var1),
                OpVal(var2)
            )?;
            writeln!(self.out, "  {} = zext i1 %t{} to i32", assign, tmp_i1)?;
        }
        Ok(())
    }

    fn gen_expr_compare_addr(
        &mut self,
        comp: &AComparator,
        var1: &Op,
        var2: &Op,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let tmp_ptr1 = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{ptr} = extractvalue %ref {op}, 0",
                op = OpVal(var1),
                ptr = tmp_ptr1
            )?;
            let tmp_ptr2 = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{ptr} = extractvalue %ref {op}, 0",
                op = OpVal(var2),
                ptr = tmp_ptr2
            )?;
            let code = match comp {
                AComparator::Eq => "eq",
                AComparator::Ne => "ne",
            };
            let tmp_i1 = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = icmp {} i8* %t{}, %t{}",
                tmp_i1, code, tmp_ptr1, tmp_ptr2
            )?;
            writeln!(self.out, "  {} = zext i1 %t{} to i32", assign, tmp_i1)?;
        }
        Ok(())
    }

    fn gen_expr_compare_long(&mut self, var1: &Op, var2: &Op, dest: Dest) -> Fallible<()> {
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

    fn gen_expr_compare_fp(
        &mut self,
        var1: &Op,
        var2: &Op,
        mode: &NaNCmpMode,
        dest: Dest,
    ) -> Fallible<()> {
        if let Dest::Assign(assign) = dest {
            let typ = tlt_type(&var1.get_type());

            let tmp_is_gt = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = fcmp ogt {typ} {}, {}",
                tmp_is_gt,
                OpVal(var1),
                OpVal(var2),
                typ = typ
            )?;

            let tmp_is_eq = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = fcmp oeq {typ} {}, {}",
                tmp_is_eq,
                OpVal(var1),
                OpVal(var2),
                typ = typ
            )?;

            let tmp_is_lt = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = fcmp olt {typ} {}, {}",
                tmp_is_lt,
                OpVal(var1),
                OpVal(var2),
                typ = typ
            )?;

            let nan_op = match mode {
                NaNCmpMode::Greater => Op::Const(Const::Int(1)),
                NaNCmpMode::Less => Op::Const(Const::Int(-1)),
            };

            let tmp_lt_or_nan = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = select i1 %t{}, i32 -1, i32 {}",
                tmp_lt_or_nan,
                tmp_is_lt,
                OpVal(&nan_op),
            )?;

            let tmp_eq_or_lt_or_nan = self.var_id_gen.gen();
            writeln!(
                self.out,
                "  %t{} = select i1 %t{}, i32 0, i32 %t{}",
                tmp_eq_or_lt_or_nan, tmp_is_eq, tmp_lt_or_nan,
            )?;

            writeln!(
                self.out,
                "  {} = select i1 %t{}, i32 1, i32 %t{}",
                assign, tmp_is_gt, tmp_eq_or_lt_or_nan,
            )?;
        }
        Ok(())
    }
}
