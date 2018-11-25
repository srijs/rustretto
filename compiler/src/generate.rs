use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use classfile::attrs::SourceFile;
use classfile::descriptors::{
    ArrayType, BaseType, FieldType, ObjectType, ParameterDescriptor, ReturnTypeDescriptor,
};
use classfile::{constant_pool::Constant, ClassFile, ConstantIndex, ConstantPool, Method};
use failure::Fallible;

use blocks::BlockGraph;
use translate::{
    BasicBlock, BranchStub, Comparator, Expr, InvokeExpr, InvokeTarget, Statement, VarId, VarIdGen,
};
use types::Type;

pub(crate) struct CodeGen {
    target_path: PathBuf,
    target_triple: String,
}

impl CodeGen {
    pub fn new(target_path: PathBuf, target_triple: String) -> Self {
        CodeGen {
            target_path,
            target_triple,
        }
    }

    pub fn generate_class(&self, class: &Arc<ClassFile>) -> Fallible<ClassCodeGen> {
        let class_name = class
            .constant_pool
            .get_utf8(class.get_this_class().name_index)
            .unwrap();
        let file = File::create(self.target_path.join(format!("{}.ll", mangle(class_name))))?;
        Ok(ClassCodeGen {
            file: BufWriter::new(file),
            class: class.clone(),
            target_triple: self.target_triple.clone(),
        })
    }
}

pub(crate) struct ClassCodeGen {
    file: BufWriter<File>,
    class: Arc<ClassFile>,
    target_triple: String,
}

impl ClassCodeGen {
    pub(crate) fn gen_main(&mut self) -> Fallible<()> {
        let class_name = self
            .class
            .constant_pool
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();
        writeln!(self.file, "define i32 @main() {{")?;
        writeln!(
            self.file,
            "  call void @{}(%ref zeroinitializer)",
            mangle_method_name(
                class_name,
                "main",
                &[ParameterDescriptor::Field(FieldType::Array(ArrayType {
                    component_type: Box::new(FieldType::Object(ObjectType {
                        class_name: "java.lang.String".to_owned()
                    }))
                }))]
            )
        )?;
        writeln!(self.file, "  ret i32 0")?;
        writeln!(self.file, "}}")?;
        Ok(())
    }

    pub(crate) fn gen_prelude(&mut self) -> Fallible<()> {
        let class_name = self
            .class
            .constant_pool
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();

        let filename = self.class.attributes.get::<SourceFile>()?;
        let target_datalayout = target_datalayout(&self.target_triple)?;

        writeln!(self.file, "; ModuleID = '{}'", self.class.get_name())?;
        writeln!(self.file, "source_filename = \"{}\"", filename.as_str())?;
        writeln!(self.file, "target datalayout = \"{}\"", target_datalayout)?;
        writeln!(self.file, "target triple = \"{}\"", self.target_triple)?;
        writeln!(self.file, "")?;

        writeln!(self.file, "%ref = type {{ i8*, i8* }}")?;

        writeln!(self.file, "declare %ref @_Jrt_ldstr(i32, i8*)")?;

        for index in self.class.constant_pool.indices() {
            match self.class.constant_pool.get_info(index).unwrap() {
                Constant::String(string_const) => {
                    let utf8_index = string_const.string_index;
                    write!(self.file, "\n")?;
                    let utf8 = self.class.constant_pool.get_utf8(utf8_index).unwrap();
                    write!(
                        self.file,
                        "@.str{} = internal constant [{} x i8] [",
                        utf8_index.as_u16(),
                        utf8.len() + 1
                    )?;
                    for byte in utf8.as_bytes() {
                        write!(self.file, " i8 {},", byte)?;
                    }
                    writeln!(self.file, " i8 0 ]")?;
                }
                Constant::MethodRef(_) => {
                    write!(self.file, "\n")?;
                    let method_ref = self.class.constant_pool.get_method_ref(index).unwrap();
                    let method_name = self
                        .class
                        .constant_pool
                        .get_utf8(method_ref.name_index)
                        .unwrap();
                    let method_class = self
                        .class
                        .constant_pool
                        .get_class(method_ref.class_index)
                        .unwrap();
                    let method_class_name = self
                        .class
                        .constant_pool
                        .get_utf8(method_class.name_index)
                        .unwrap();
                    // Skip methods of the current class
                    if method_class_name == class_name {
                        continue;
                    }
                    write!(
                        self.file,
                        "declare {return_type} @{mangled_name}(",
                        return_type = tlt_return_type(&method_ref.descriptor.ret),
                        mangled_name = mangle_method_name(
                            method_class_name,
                            method_name,
                            &method_ref.descriptor.params
                        )
                    )?;
                    write!(self.file, "%ref")?;
                    for ParameterDescriptor::Field(field) in method_ref.descriptor.params.iter() {
                        write!(self.file, ", {}", tlt_field_type(field))?;
                    }
                    writeln!(self.file, ")")?;
                }
                Constant::FieldRef(_) => {
                    write!(self.file, "\n")?;
                    let field_ref = self.class.constant_pool.get_field_ref(index).unwrap();
                    let field_name = self
                        .class
                        .constant_pool
                        .get_utf8(field_ref.name_index)
                        .unwrap();
                    let field_class = self
                        .class
                        .constant_pool
                        .get_class(field_ref.class_index)
                        .unwrap();
                    let field_class_name = self
                        .class
                        .constant_pool
                        .get_utf8(field_class.name_index)
                        .unwrap();
                    writeln!(
                        self.file,
                        "declare {field_type} @{mangled_name}__get(%ref)",
                        field_type = tlt_field_type(&field_ref.descriptor),
                        mangled_name = mangle_field_name(field_class_name, field_name)
                    )?;
                    writeln!(
                        self.file,
                        "declare void @{mangled_name}__set(%ref, {field_type})",
                        field_type = tlt_field_type(&field_ref.descriptor),
                        mangled_name = mangle_field_name(field_class_name, field_name)
                    )?;
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
        var_id_gen: &mut VarIdGen,
    ) -> Fallible<()> {
        let class_name = consts
            .get_utf8(self.class.get_this_class().name_index)
            .unwrap();
        let method_name = consts.get_utf8(method.name_index).unwrap();
        write!(
            self.file,
            "\ndefine {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method.descriptor.ret),
            mangled_name = mangle_method_name(class_name, method_name, &method.descriptor.params)
        )?;
        for (i, (_, var)) in blocks.lookup(0).incoming.locals.iter().enumerate() {
            if i > 0 {
                write!(self.file, ", ")?;
            }
            write!(self.file, "{} %v{}", tlt_type(&var.0), var.1)?;
        }
        writeln!(self.file, ") {{")?;
        for block in blocks.blocks() {
            self.gen_block(block, blocks, consts, var_id_gen)?;
        }
        writeln!(self.file, "}}")?;
        Ok(())
    }

    fn gen_block(
        &mut self,
        block: &BasicBlock,
        blocks: &BlockGraph,
        consts: &ConstantPool,
        var_id_gen: &mut VarIdGen,
    ) -> Fallible<()> {
        writeln!(self.file, "B{}:", block.address)?;
        self.gen_phi_nodes(block, blocks)?;
        for stmt in block.statements.iter() {
            self.gen_statement(stmt, consts)?;
        }
        match &block.branch_stub {
            BranchStub::Goto(addr) => writeln!(self.file, "  br label %B{}", addr)?,
            BranchStub::Return(None) => writeln!(self.file, "  ret void")?,
            BranchStub::IfICmp(comp, var1, var2_opt, if_addr, else_addr) => {
                let tmp = var_id_gen.gen(Type::int());
                let code = match comp {
                    Comparator::Eq => "eq",
                    Comparator::Ge => "sge",
                };
                if let Some(var2) = var2_opt {
                    writeln!(
                        self.file,
                        "  %tmp{} = icmp {} i32 %v{}, %v{}",
                        tmp.1, code, var1.1, var2.1
                    )?;
                } else {
                    writeln!(
                        self.file,
                        "  %tmp{} = icmp {} i32 0, %v{}",
                        tmp.1, code, var1.1
                    )?;
                }
                writeln!(
                    self.file,
                    "  br i1 %tmp{}, label %B{}, label %B{}",
                    tmp.1, if_addr, else_addr
                )?;
            }
            _ => unimplemented!(),
        }
        Ok(())
    }

    fn gen_statement(&mut self, stmt: &Statement, consts: &ConstantPool) -> Fallible<()> {
        if let Some(ref var) = stmt.assign {
            write!(self.file, "  %v{} = ", var.1)?;
        } else {
            write!(self.file, "  ")?;
        }
        self.gen_expr(&stmt.expression, consts)
    }

    fn gen_expr(&mut self, expr: &Expr, consts: &ConstantPool) -> Fallible<()> {
        match expr {
            Expr::ConstInt(i) => writeln!(self.file, "and i32 {}, {}", i, i)?,
            Expr::ConstString(index) => self.gen_load_string(*index, consts)?,
            Expr::GetStatic(index) => self.gen_expr_get_static(*index, consts)?,
            Expr::Invoke(subexpr) => self.gen_expr_invoke(subexpr, consts)?,
            Expr::IInc(var, i) => writeln!(self.file, "add i32 %v{}, {}", var.1, i)?,
            _ => bail!("unknown expression {:?}", expr),
        }
        Ok(())
    }

    fn gen_load_string(&mut self, index: ConstantIndex, consts: &ConstantPool) -> Fallible<()> {
        let len = consts.get_utf8(index).unwrap().len();
        writeln!(
            self.file,
            "call %ref @_Jrt_ldstr(i32 {}, i8* getelementptr ([{} x i8], [{} x i8]* @.str{}, i64 0, i64 0))",
            len,
            len + 1,
            len + 1,
            index.as_u16()
        )?;
        Ok(())
    }

    fn gen_expr_invoke(&mut self, expr: &InvokeExpr, consts: &ConstantPool) -> Fallible<()> {
        let method_ref = consts.get_method_ref(expr.index).unwrap();
        let method_name = consts.get_utf8(method_ref.name_index).unwrap();
        let method_class = consts.get_class(method_ref.class_index).unwrap();
        let method_class_name = consts.get_utf8(method_class.name_index).unwrap();
        write!(
            self.file,
            "call {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method_ref.descriptor.ret),
            mangled_name = mangle_method_name(
                method_class_name,
                method_name,
                &method_ref.descriptor.params
            )
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
                write!(self.file, ", {}", arg)?;
            } else {
                write!(self.file, "{}", arg)?;
            }
        }

        writeln!(self.file, ")")?;
        Ok(())
    }

    fn gen_expr_get_static(&mut self, index: ConstantIndex, consts: &ConstantPool) -> Fallible<()> {
        let field_ref = consts.get_field_ref(index).unwrap();
        let field_name = consts.get_utf8(field_ref.name_index).unwrap();
        let field_class = consts.get_class(field_ref.class_index).unwrap();
        let field_class_name = consts.get_utf8(field_class.name_index).unwrap();
        writeln!(
            self.file,
            "call {field_type} @{mangled_name}__get(%ref zeroinitializer)",
            field_type = tlt_field_type(&field_ref.descriptor),
            mangled_name = mangle_field_name(field_class_name, field_name)
        )?;
        Ok(())
    }

    fn gen_phi_nodes(&mut self, block: &BasicBlock, blocks: &BlockGraph) -> Fallible<()> {
        let mut phis = BTreeMap::<VarId, Vec<(VarId, u32)>>::new();
        for incoming_block in blocks.incoming(block.address) {
            for (i, out_var) in incoming_block.outgoing.stack.iter().enumerate() {
                let var = &block.incoming.stack[i];
                phis.entry(var.clone())
                    .or_default()
                    .push((out_var.clone(), incoming_block.address));
            }
            for (i, out_var) in incoming_block.outgoing.locals.iter() {
                let var = &block.incoming.locals[i];
                phis.entry(var.clone())
                    .or_default()
                    .push((out_var.clone(), incoming_block.address));
            }
        }
        for (var, bindings) in phis {
            write!(self.file, "  %v{} = phi {} ", var.1, tlt_type(&var.0))?;
            for (i, (out_var, addr)) in bindings.iter().enumerate() {
                if i > 0 {
                    write!(self.file, ", ")?;
                }
                write!(self.file, "[ %v{}, %B{} ]", out_var.1, addr)?;
            }
            writeln!(self.file, "")?;
        }
        Ok(())
    }
}

fn target_datalayout(target_triple: &str) -> Fallible<&'static str> {
    match target_triple {
        "x86_64-apple-darwin" => Ok("e-m:o-i64:64-f80:128-n8:16:32:64-S128"),
        _ => bail!("could not determine data layout: unknown target triple"),
    }
}

fn mangle_field_name(class_name: &str, field_name: &str) -> String {
    format!("_Jf_{}_{}", class_name.replace("/", "_"), field_name)
}

fn mangle_method_name(
    class_name: &str,
    method_name: &str,
    params: &[ParameterDescriptor],
) -> String {
    let mangled_class_name = mangle(class_name);
    let mangled_method_name = match method_name {
        "<init>" => "_init".to_owned(),
        _ => mangle(method_name),
    };
    let mut mangled = format!("_Jm_{}_{}", mangled_class_name, mangled_method_name);
    if params.len() > 0 {
        mangled.push_str("__");
        for ParameterDescriptor::Field(field_type) in params {
            mangled.push_str(&mangle(&field_type.to_string()));
        }
    }
    return mangled;
}

fn mangle(input: &str) -> String {
    let mut output = input.to_owned();
    output = output.replace("_", "_1");
    output = output.replace(";", "_2");
    output = output.replace("[", "_3");
    output = output.replace("/", "_");
    output = output.replace(".", "_");
    return output;
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
