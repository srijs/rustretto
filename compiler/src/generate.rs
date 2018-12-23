use std::fmt::Write;
use std::sync::Arc;

use classfile::attrs::SourceFile;
use classfile::constant_pool::Constant;
use classfile::descriptors::{
    ArrayType, BaseType, FieldType, MethodDescriptor, ObjectType, ParameterDescriptor,
    ReturnTypeDescriptor,
};
use classfile::{ClassFile, ConstantIndex, ConstantPool, Method};
use failure::{bail, Fallible};
use llvm::codegen::{TargetDataLayout, TargetMachine, TargetTriple};
use strbuf::StrBuf;

use crate::blocks::BlockGraph;
use crate::classes::ClassGraph;
use crate::loader::Class;
use crate::mangle;
use crate::translate::{
    AComparator, BasicBlock, BlockId, BranchStub, Const, Expr, IComparator, InvokeExpr,
    InvokeTarget, Statement, Switch, VarId,
};
use crate::types::Type;
use crate::vtable::VTableMap;

pub(crate) struct CodeGen {
    classes: ClassGraph,
    vtables: VTableMap,
    machine: Arc<TargetMachine>,
    data_layout: TargetDataLayout,
}

impl CodeGen {
    pub fn new(classes: ClassGraph, machine: Arc<TargetMachine>) -> Fallible<Self> {
        let vtables = VTableMap::new(classes.clone());
        let data_layout = machine.data_layout();
        Ok(CodeGen {
            classes,
            vtables,
            machine,
            data_layout,
        })
    }

    pub fn generate_class(&self, name: &str) -> Fallible<ClassCodeGen> {
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
            class: class.clone(),
            classes: self.classes.clone(),
            vtables: self.vtables.clone(),
            var_id_gen: TmpVarIdGen::new(),
            target_triple: self.machine.triple(),
            target_data_layout: self.data_layout.to_string_rep(),
        })
    }
}

pub(crate) struct ClassCodeGen {
    out: String,
    class: Arc<ClassFile>,
    classes: ClassGraph,
    vtables: VTableMap,
    var_id_gen: TmpVarIdGen,
    target_triple: TargetTriple,
    target_data_layout: llvm::Message,
}

impl ClassCodeGen {
    pub(crate) fn finish(self) -> String {
        self.out
    }

    pub(crate) fn gen_main(&mut self) -> Fallible<()> {
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

    pub(crate) fn gen_vtable_type(&mut self, class_name: &StrBuf) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;
        writeln!(
            self.out,
            "%{} = type {{",
            mangle::mangle_vtable_name(class_name)
        )?;
        for (idx, (key, _)) in vtable.iter().enumerate() {
            let ftyp = tlt_function_type(&key.method_descriptor);
            write!(self.out, "  {} *", ftyp)?;
            if idx < vtable.len() - 1 {
                write!(self.out, ",")?;
            } else {
                write!(self.out, "")?;
            }
            writeln!(self.out, " ; {}", key.method_name)?;
        }
        writeln!(self.out, "}}")?;

        Ok(())
    }

    pub(crate) fn gen_vtable_const(&mut self, class_name: &StrBuf) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;
        let vtable_name = mangle::mangle_vtable_name(class_name);

        writeln!(
            self.out,
            "@{vtable} = constant %{vtable} {{",
            vtable = vtable_name
        )?;
        for (idx, (key, target)) in vtable.iter().enumerate() {
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
                writeln!(self.out, "")?;
            }
        }
        writeln!(self.out, "}}")?;

        Ok(())
    }

    pub(crate) fn gen_vtable_decls(&mut self, class_name: &StrBuf) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;

        for (key, target) in vtable.iter() {
            if target.class_name == *class_name {
                continue;
            }
            write!(
                self.out,
                "declare {return_type} @{mangled_name}(",
                return_type = tlt_return_type(&key.method_descriptor.ret),
                mangled_name = mangle::mangle_method_name(
                    &target.class_name,
                    &key.method_name,
                    &key.method_descriptor.ret,
                    &key.method_descriptor.params
                )
            )?;
            write!(self.out, "%ref")?;
            for ParameterDescriptor::Field(field) in key.method_descriptor.params.iter() {
                write!(self.out, ", {}", tlt_field_type(field))?;
            }
            writeln!(self.out, ")")?;
        }

        Ok(())
    }

    pub(crate) fn gen_extern_decls(&mut self, class: &ClassFile) -> Fallible<()> {
        let class_name = class.get_name();
        let vtable_name = mangle::mangle_vtable_name(class_name);

        writeln!(
            self.out,
            "@{vtable} = external global %{vtable}",
            vtable = vtable_name
        )?;

        for method in class.methods.iter() {
            let method_name = class.constant_pool.get_utf8(method.name_index).unwrap();

            if &**method_name != "<init>" && !method.is_static() {
                continue;
            }

            let mut args = vec![];
            if !method.is_static() {
                args.push(Type::UninitializedThis);
            }
            for ParameterDescriptor::Field(field_type) in method.descriptor.params.iter() {
                args.push(Type::from_field_type(field_type.clone()));
            }

            write!(
                self.out,
                "declare {return_type} @{mangled_name}(",
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
                write!(self.out, "{}", tlt_type(arg))?;
            }
            writeln!(self.out, ")")?;
        }

        for field in class.fields.iter() {
            let field_name = class.constant_pool.get_utf8(field.name_index).unwrap();
            writeln!(
                self.out,
                "declare {field_type} @{mangled_name}(%ref)",
                field_type = tlt_field_type(&field.descriptor),
                mangled_name = mangle::mangle_field_name_getter(class_name, field_name)
            )?;
            writeln!(
                self.out,
                "declare void @{mangled_name}(%ref, {field_type})",
                field_type = tlt_field_type(&field.descriptor),
                mangled_name = mangle::mangle_field_name_setter(class_name, field_name)
            )?;
        }

        Ok(())
    }

    pub(crate) fn gen_prelude(&mut self) -> Fallible<()> {
        let filename = self.class.attributes.get::<SourceFile>()?;

        writeln!(self.out, "; ModuleID = '{}'", self.class.get_name())?;
        writeln!(self.out, "source_filename = \"{}\"", filename.as_str())?;
        writeln!(
            self.out,
            "target datalayout = \"{}\"",
            self.target_data_layout
        )?;
        writeln!(self.out, "target triple = \"{}\"", self.target_triple)?;
        writeln!(self.out, "")?;

        writeln!(self.out, "%ref = type {{ i8*, i8* }}")?;

        writeln!(self.out, "declare void @_Jrt_throw(%ref) noreturn")?;
        writeln!(self.out, "declare %ref @_Jrt_ldstr(i32, i8*)")?;

        for index in self.class.constant_pool.indices() {
            match self.class.constant_pool.get_info(index).unwrap() {
                Constant::String(string_const) => {
                    let utf8_index = string_const.string_index;
                    write!(self.out, "\n")?;
                    let utf8 = self.class.constant_pool.get_utf8(utf8_index).unwrap();
                    write!(
                        self.out,
                        "@.str{} = internal constant [{} x i8] [",
                        utf8_index.as_u16(),
                        utf8.len() + 1
                    )?;
                    for byte in utf8.as_bytes() {
                        write!(self.out, " i8 {},", byte)?;
                    }
                    writeln!(self.out, " i8 0 ]")?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn gen_method(
        &mut self,
        method: &Method,
        blocks: &BlockGraph,
        consts: &ConstantPool,
    ) -> Fallible<()> {
        let class_name = consts
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();
        let method_name = consts.get_utf8(method.name_index).unwrap();
        write!(
            self.out,
            "\ndefine {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method.descriptor.ret),
            mangled_name = mangle::mangle_method_name(
                class_name,
                method_name,
                &method.descriptor.ret,
                &method.descriptor.params
            )
        )?;
        for (i, (_, var)) in blocks
            .lookup(BlockId::start())
            .incoming
            .locals
            .iter()
            .enumerate()
        {
            if i > 0 {
                write!(self.out, ", ")?;
            }
            write!(self.out, "{} %v{}", tlt_type(&var.0), var.1)?;
        }
        writeln!(self.out, ") {{")?;
        for block in blocks.blocks() {
            self.gen_block(block, blocks, consts)?;
        }
        writeln!(self.out, "}}")?;
        Ok(())
    }

    fn gen_block(
        &mut self,
        block: &BasicBlock,
        blocks: &BlockGraph,
        consts: &ConstantPool,
    ) -> Fallible<()> {
        writeln!(self.out, "B{}:", block.address)?;
        self.gen_phi_nodes(block, blocks)?;
        for stmt in block.statements.iter() {
            self.gen_statement(stmt, consts)?;
        }
        match &block.branch_stub {
            BranchStub::Goto(addr) => writeln!(self.out, "  br label %B{}", addr)?,
            BranchStub::Return(ret_opt) => {
                if let Some(ret) = ret_opt {
                    writeln!(self.out, "  ret {} %v{}", tlt_type(&ret.0), ret.1)?;
                } else {
                    writeln!(self.out, "  ret void")?;
                }
            }
            BranchStub::Switch(switch) => self.gen_switch(switch)?,
            BranchStub::IfICmp(comp, var1, var2_opt, if_addr, else_addr) => {
                self.gen_icmp(comp, var1, var2_opt, *if_addr, *else_addr)?
            }
            BranchStub::IfACmp(comp, var1, var2, if_addr, else_addr) => {
                self.gen_acmp(comp, var1, var2, *if_addr, *else_addr)?
            }
            BranchStub::Throw(var) => {
                writeln!(
                    self.out,
                    "  call void @_Jrt_throw(%ref %v{}) noreturn",
                    var.1
                )?;
                writeln!(self.out, "  unreachable")?;
            }
        }
        Ok(())
    }

    fn gen_icmp(
        &mut self,
        comp: &IComparator,
        var1: &VarId,
        var2_opt: &Option<VarId>,
        if_addr: BlockId,
        else_addr: BlockId,
    ) -> Fallible<()> {
        let tmpid = self.var_id_gen.gen();
        let code = match comp {
            IComparator::Eq => "eq",
            IComparator::Ge => "sge",
            IComparator::Le => "sle",
            IComparator::Lt => "slt",
        };
        if let Some(var2) = var2_opt {
            writeln!(
                self.out,
                "  %tmp{} = icmp {} i32 %v{}, %v{}",
                tmpid, code, var1.1, var2.1
            )?;
        } else {
            writeln!(
                self.out,
                "  %tmp{} = icmp {} i32 0, %v{}",
                tmpid, code, var1.1
            )?;
        }
        writeln!(
            self.out,
            "  br i1 %tmp{}, label %B{}, label %B{}",
            tmpid, if_addr, else_addr
        )?;
        Ok(())
    }

    fn gen_acmp(
        &mut self,
        comp: &AComparator,
        var1: &VarId,
        var2: &VarId,
        if_addr: BlockId,
        else_addr: BlockId,
    ) -> Fallible<()> {
        let tmp_ptr1 = self.var_id_gen.gen();
        self.gen_ref_to_ptr_to_int(var1, tmp_ptr1)?;
        let tmp_ptr2 = self.var_id_gen.gen();
        self.gen_ref_to_ptr_to_int(var2, tmp_ptr2)?;

        let code = match comp {
            AComparator::Eq => "eq",
            AComparator::Ne => "ne",
        };

        let tmp_cmp = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{} = icmp {} i64 %t{}, %t{}",
            tmp_cmp, code, tmp_ptr1, tmp_ptr2
        )?;
        writeln!(
            self.out,
            "  br i1 %t{}, label %B{}, label %B{}",
            tmp_cmp, if_addr, else_addr
        )?;
        Ok(())
    }

    fn gen_ref_to_ptr_to_int(&mut self, var: &VarId, dest_tmpvar: u64) -> Fallible<()> {
        let tmp_ptr = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{ptr} = extractvalue %ref %v{var}, 1",
            var = var.1,
            ptr = tmp_ptr
        )?;

        writeln!(
            self.out,
            "  %t{dest} = ptrtoint i8* %t{ptr} to i64",
            ptr = tmp_ptr,
            dest = dest_tmpvar
        )?;

        Ok(())
    }

    fn gen_switch(&mut self, switch: &Switch) -> Fallible<()> {
        write!(
            self.out,
            "  switch i32 %v{}, label %B{} [",
            switch.value.1, switch.default
        )?;
        for (value, addr) in switch.cases.iter() {
            write!(self.out, " i32 {}, label %B{}", value, addr)?;
        }
        writeln!(self.out, " ]")?;
        Ok(())
    }

    fn gen_statement(&mut self, stmt: &Statement, consts: &ConstantPool) -> Fallible<()> {
        let dest;
        if let Some(ref var) = stmt.assign {
            dest = Dest::Assign(var.clone());
        } else {
            dest = Dest::Ignore;
        }
        self.gen_expr(&stmt.expression, consts, dest)
    }

    fn gen_expr(&mut self, expr: &Expr, consts: &ConstantPool, dest: Dest) -> Fallible<()> {
        match expr {
            Expr::Const(Const::Int(i)) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.out, "  %v{} = and i32 {}, {}", dest_var.1, i, i)?;
                }
            }
            Expr::Const(Const::Long(i)) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.out, "  %v{} = and i64 {}, {}", dest_var.1, i, i)?;
                }
            }
            Expr::Const(Const::String(index)) => self.gen_load_string(*index, consts, dest)?,
            Expr::GetStatic(index) => self.gen_expr_get_static(*index, consts, dest)?,
            Expr::Invoke(subexpr) => self.gen_expr_invoke(subexpr, consts, dest)?,
            Expr::IInc(var, i) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.out, "  %v{} = add i32 %v{}, {}", dest_var.1, var.1, i)?;
                }
            }
            Expr::LAdd(var1, var2) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(
                        self.out,
                        "  %v{} = add i64 %v{}, %v{}",
                        dest_var.1, var1.1, var2.1
                    )?;
                }
            }
            Expr::LCmp(var1, var2) => self.gen_cmp_long(var1, var2, dest)?,
            Expr::New(class_name) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.out, "  %v{} = insertvalue %ref zeroinitializer, i8* bitcast (%{vtable}* @{vtable} to i8*), 1", dest_var.1, vtable = mangle::mangle_vtable_name(class_name))?;
                }
            }
            _ => bail!("unknown expression {:?}", expr),
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
        if let Dest::Assign(dest_var) = dest {
            writeln!(
                self.out,
                "  %v{} = call %ref @_Jrt_ldstr(i32 {}, i8* getelementptr ([{} x i8], [{} x i8]* @.str{}, i64 0, i64 0))",
                dest_var.1,
                len,
                len + 1,
                len + 1,
                index.as_u16()
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
        let vtable_name = mangle::mangle_vtable_name(method_class_name);

        /*
        %v9vtblraw = extractvalue %ref %v9, 1
        %v9vtbl = bitcast i8* %v9vtblraw to %vtable.java_io_PrintStream*
        %v9fptrptr = getelementptr %vtable.java_io_PrintStream, %vtable.java_io_PrintStream* %v9vtbl, i64 0, i32 13
        %v9fptr = load void (%ref, %ref)*, void (%ref, %ref)** %v9fptrptr
        call void %v9fptr(%ref %v9, %ref %v10)
        */

        let fptr = match expr.target {
            InvokeTarget::Virtual(ref var) => {
                let vtable = self.vtables.get(method_class_name)?;
                let (offset, _) = vtable.get(method_name, &method_ref.descriptor).unwrap();

                writeln!(self.out, "  ; prepare virtual dispatch")?;
                let tmp_vtblraw = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{vtblraw} = extractvalue %ref %v{}, 1",
                    var.1,
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_vtbl = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{vtbl} = bitcast i8* %t{vtblraw} to %{vtblnm}*",
                    vtblnm = vtable_name,
                    vtbl = tmp_vtbl,
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_fptrptr = self.var_id_gen.gen();
                writeln!(
                    self.out,
                    "  %t{fptrptr} = getelementptr %{vtblnm}, %{vtblnm}* %t{vtbl}, i64 0, i32 {offset}",
                    offset = offset,
                    vtblnm = vtable_name,
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

        if let Dest::Assign(dest_var) = dest {
            write!(self.out, "  %v{} = ", dest_var.1)?;
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
            InvokeTarget::Special(ref var) => args.push(format!("%ref %v{}", var.1)),
            InvokeTarget::Virtual(ref var) => args.push(format!("%ref %v{}", var.1)),
        };

        for var in expr.args.iter() {
            args.push(format!("{} %v{}", tlt_type(&var.0), var.1));
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
        if let Dest::Assign(dest_var) = dest {
            writeln!(
                self.out,
                "  %v{} = call {field_type} @{mangled_name}(%ref zeroinitializer)",
                dest_var.1,
                field_type = tlt_field_type(&field_ref.descriptor),
                mangled_name = mangle::mangle_field_name_getter(field_class_name, field_name)
            )?;
        }
        Ok(())
    }

    fn gen_phi_nodes(&mut self, block: &BasicBlock, blocks: &BlockGraph) -> Fallible<()> {
        let phis = blocks.phis(block);
        for (var, bindings) in phis {
            write!(self.out, "  %v{} = phi {} ", var.1, tlt_type(&var.0))?;
            for (i, (out_var, addr)) in bindings.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ")?;
                }
                write!(self.out, "[ %v{}, %B{} ]", out_var.1, addr)?;
            }
            writeln!(self.out, "")?;
        }
        Ok(())
    }

    fn gen_cmp_long(&mut self, var1: &VarId, var2: &VarId, dest: Dest) -> Fallible<()> {
        let tmp_lt = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{} = icmp slt i64 %v{}, %v{}",
            tmp_lt, var1.1, var2.1
        )?;
        let tmp_lt_ext = self.var_id_gen.gen();
        writeln!(self.out, "  %t{} = zext i1 %t{} to i32", tmp_lt_ext, tmp_lt)?;
        let tmp_gt = self.var_id_gen.gen();
        writeln!(
            self.out,
            "  %t{} = icmp sgt i64 %v{}, %v{}",
            tmp_gt, var1.1, var2.1
        )?;
        let tmp_gt_ext = self.var_id_gen.gen();
        writeln!(self.out, "  %t{} = zext i1 %t{} to i32", tmp_gt_ext, tmp_gt)?;
        if let Dest::Assign(dest_var) = dest {
            writeln!(
                self.out,
                "  %v{} = sub i32 %t{}, %t{}",
                dest_var.1, tmp_lt_ext, tmp_gt_ext
            )?;
        }
        Ok(())
    }

    pub(crate) fn gen_native_method(
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
}

enum Dest {
    Ignore,
    Assign(VarId),
}

pub struct TmpVarIdGen {
    next_id: u64,
}

impl TmpVarIdGen {
    pub fn new() -> Self {
        TmpVarIdGen { next_id: 0 }
    }

    pub fn gen(&mut self) -> u64 {
        let var_id = self.next_id;
        self.next_id += 1;
        var_id
    }
}

fn tlt_function_type(descr: &MethodDescriptor) -> String {
    let mut output = tlt_return_type(&descr.ret).to_owned();
    output.push_str(" (%ref");
    for ParameterDescriptor::Field(field) in descr.params.iter() {
        output.push_str(", ");
        output.push_str(tlt_field_type(field));
    }
    output.push_str(")");
    output
}

fn tlt_return_type(return_type: &ReturnTypeDescriptor) -> &'static str {
    match return_type {
        ReturnTypeDescriptor::Void => "void",
        ReturnTypeDescriptor::Field(field_type) => tlt_field_type(field_type),
    }
}

fn tlt_field_type(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Base(base_type) => match base_type {
            BaseType::Boolean => "i32",
            BaseType::Byte => "i32",
            BaseType::Char => "i32",
            BaseType::Short => "i32",
            BaseType::Int => "i32",
            BaseType::Long => "i64",
            BaseType::Float => "float",
            BaseType::Double => "double",
        },
        FieldType::Object(_) | FieldType::Array(_) => "%ref",
    }
}

fn tlt_type(t: &Type) -> &'static str {
    match t {
        Type::Integer => "i32",
        Type::Long => "i64",
        Type::Float => "float",
        Type::Double => "double",
        Type::Null | Type::Object(_) | Type::UninitializedThis => "%ref",
        _ => unimplemented!(),
    }
}
