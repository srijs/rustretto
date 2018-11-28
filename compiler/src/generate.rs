use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use classfile::attrs::SourceFile;
use classfile::descriptors::{
    ArrayType, BaseType, FieldType, MethodDescriptor, ObjectType, ParameterDescriptor,
    ReturnTypeDescriptor,
};
use classfile::{constant_pool::Constant, ClassFile, ConstantIndex, ConstantPool, Method};
use failure::Fallible;

use blocks::BlockGraph;
use classes::ClassGraph;
use loader::Class;
use translate::{
    BasicBlock, BranchStub, Comparator, Expr, InvokeExpr, InvokeTarget, Statement, VarId,
};
use types::Type;
use vtable::VTableMap;

pub(crate) struct CodeGen {
    classes: ClassGraph,
    vtables: VTableMap,
    target_path: PathBuf,
    target_triple: String,
}

impl CodeGen {
    pub fn new(classes: ClassGraph, target_path: PathBuf, target_triple: String) -> Self {
        let vtables = VTableMap::new(classes.clone());
        CodeGen {
            classes,
            vtables,
            target_path,
            target_triple,
        }
    }

    pub fn generate_class(&self, name: &str) -> Fallible<ClassCodeGen> {
        let class = match self.classes.get(name)? {
            Class::File(class_file) => class_file,
            _ => bail!("can't generate code for array class"),
        };
        let class_name = class
            .constant_pool
            .get_utf8(class.get_this_class().name_index)
            .unwrap();
        let file = File::create(self.target_path.join(format!("{}.ll", mangle(class_name))))?;
        Ok(ClassCodeGen {
            file: BufWriter::new(file),
            class: class.clone(),
            classes: self.classes.clone(),
            vtables: self.vtables.clone(),
            target_triple: self.target_triple.clone(),
            var_id_gen: TmpVarIdGen::new(),
        })
    }
}

pub(crate) struct ClassCodeGen {
    file: BufWriter<File>,
    class: Arc<ClassFile>,
    classes: ClassGraph,
    vtables: VTableMap,
    target_triple: String,
    var_id_gen: TmpVarIdGen,
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
                &ReturnTypeDescriptor::Void,
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

    pub(crate) fn gen_vtable_type(&mut self, class_name: &str) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;
        writeln!(self.file, "%vtable.{} = type {{", mangle(class_name))?;
        for (idx, (key, _)) in vtable.iter().enumerate() {
            let ftyp = tlt_function_type(&key.method_descriptor);
            write!(self.file, "  {} *", ftyp)?;
            if idx < vtable.len() - 1 {
                write!(self.file, ",")?;
            } else {
                write!(self.file, "")?;
            }
            writeln!(self.file, " ; {}", key.method_name)?;
        }
        writeln!(self.file, "}}")?;

        Ok(())
    }

    pub(crate) fn gen_vtable_const(&mut self, class_name: &str) -> Fallible<()> {
        let vtable = self.vtables.get(class_name)?;
        let mangled_class_name = mangle(class_name);

        writeln!(
            self.file,
            "@vtable.{} = constant %vtable.{} {{",
            mangled_class_name, mangled_class_name
        )?;
        for (idx, (key, target)) in vtable.iter().enumerate() {
            write!(
                self.file,
                "  {} * @{}",
                tlt_function_type(&key.method_descriptor),
                mangle_method_name(
                    &target.class_name,
                    &key.method_name,
                    &key.method_descriptor.ret,
                    &key.method_descriptor.params
                )
            )?;
            if idx < vtable.len() - 1 {
                writeln!(self.file, ",")?;
            } else {
                writeln!(self.file, "")?;
            }
        }
        writeln!(self.file, "}}")?;

        Ok(())
    }

    pub(crate) fn gen_extern_decls(&mut self, class: &ClassFile) -> Fallible<()> {
        let class_name = class.get_name();

        for method in class.methods.iter() {
            let method_name = class.constant_pool.get_utf8(method.name_index).unwrap();

            write!(
                self.file,
                "declare {return_type} @{mangled_name}(",
                return_type = tlt_return_type(&method.descriptor.ret),
                mangled_name = mangle_method_name(
                    class_name,
                    method_name,
                    &method.descriptor.ret,
                    &method.descriptor.params
                )
            )?;
            write!(self.file, "%ref")?;
            for ParameterDescriptor::Field(field) in method.descriptor.params.iter() {
                write!(self.file, ", {}", tlt_field_type(field))?;
            }
            writeln!(self.file, ")")?;
        }

        for field in class.fields.iter() {
            let field_name = class.constant_pool.get_utf8(field.name_index).unwrap();
            writeln!(
                self.file,
                "declare {field_type} @{mangled_name}__get(%ref)",
                field_type = tlt_field_type(&field.descriptor),
                mangled_name = mangle_field_name(class_name, field_name)
            )?;
            writeln!(
                self.file,
                "declare void @{mangled_name}__set(%ref, {field_type})",
                field_type = tlt_field_type(&field.descriptor),
                mangled_name = mangle_field_name(class_name, field_name)
            )?;
        }

        Ok(())
    }

    pub(crate) fn gen_prelude(&mut self) -> Fallible<()> {
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
            self.file,
            "\ndefine {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method.descriptor.ret),
            mangled_name = mangle_method_name(
                class_name,
                method_name,
                &method.descriptor.ret,
                &method.descriptor.params
            )
        )?;
        for (i, (_, var)) in blocks.lookup(0).incoming.locals.iter().enumerate() {
            if i > 0 {
                write!(self.file, ", ")?;
            }
            write!(self.file, "{} %v{}", tlt_type(&var.0), var.1)?;
        }
        writeln!(self.file, ") {{")?;
        for block in blocks.blocks() {
            self.gen_block(block, blocks, consts)?;
        }
        writeln!(self.file, "}}")?;
        Ok(())
    }

    fn gen_block(
        &mut self,
        block: &BasicBlock,
        blocks: &BlockGraph,
        consts: &ConstantPool,
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
                let tmpid = self.var_id_gen.gen();
                let code = match comp {
                    Comparator::Eq => "eq",
                    Comparator::Ge => "sge",
                };
                if let Some(var2) = var2_opt {
                    writeln!(
                        self.file,
                        "  %tmp{} = icmp {} i32 %v{}, %v{}",
                        tmpid, code, var1.1, var2.1
                    )?;
                } else {
                    writeln!(
                        self.file,
                        "  %tmp{} = icmp {} i32 0, %v{}",
                        tmpid, code, var1.1
                    )?;
                }
                writeln!(
                    self.file,
                    "  br i1 %tmp{}, label %B{}, label %B{}",
                    tmpid, if_addr, else_addr
                )?;
            }
            _ => unimplemented!(),
        }
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
            Expr::ConstInt(i) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.file, "  %v{} = and i32 {}, {}", dest_var.1, i, i)?;
                }
            }
            Expr::ConstString(index) => self.gen_load_string(*index, consts, dest)?,
            Expr::GetStatic(index) => self.gen_expr_get_static(*index, consts, dest)?,
            Expr::Invoke(subexpr) => self.gen_expr_invoke(subexpr, consts, dest)?,
            Expr::IInc(var, i) => {
                if let Dest::Assign(dest_var) = dest {
                    writeln!(self.file, "  %v{} = add i32 %v{}, {}", dest_var.1, var.1, i)?;
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
                self.file,
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
        let mangled_method_class_name = mangle(method_class_name);

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

                writeln!(self.file, "  ; prepare virtual dispatch")?;
                let tmp_vtblraw = self.var_id_gen.gen();
                writeln!(
                    self.file,
                    "  %t{vtblraw} = extractvalue %ref %v{}, 1",
                    var.1,
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_vtbl = self.var_id_gen.gen();
                writeln!(
                    self.file,
                    "  %t{vtbl} = bitcast i8* %t{vtblraw} to %vtable.{class}*",
                    class = mangled_method_class_name,
                    vtbl = tmp_vtbl,
                    vtblraw = tmp_vtblraw
                )?;
                let tmp_fptrptr = self.var_id_gen.gen();
                writeln!(
                    self.file,
                    "  %t{fptrptr} = getelementptr %vtable.{class}, %vtable.{class}* %t{vtbl}, i64 0, i32 {offset}",
                    offset = offset,
                    class = mangled_method_class_name,
                    vtbl = tmp_vtbl,
                    fptrptr = tmp_fptrptr
                )?;
                let tmp_fptr = self.var_id_gen.gen();
                writeln!(
                    self.file,
                    "  %t{fptr} = load {ftyp}*, {ftyp}** %t{fptrptr}",
                    fptr = tmp_fptr,
                    fptrptr = tmp_fptrptr,
                    ftyp = tlt_function_type(&method_ref.descriptor)
                )?;
                writeln!(self.file, "  ; invoke {}", method_name)?;

                format!("%t{}", tmp_fptr)
            }
            _ => format!(
                "@{}",
                mangle_method_name(
                    method_class_name,
                    method_name,
                    &method_ref.descriptor.ret,
                    &method_ref.descriptor.params
                )
            ),
        };

        if let Dest::Assign(dest_var) = dest {
            write!(self.file, "  %v{} = ", dest_var.1)?;
        } else {
            write!(self.file, "  ")?;
        }

        write!(
            self.file,
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
                write!(self.file, ", {}", arg)?;
            } else {
                write!(self.file, "{}", arg)?;
            }
        }

        writeln!(self.file, ")")?;
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
                self.file,
                "  %v{} = call {field_type} @{mangled_name}__get(%ref zeroinitializer)",
                dest_var.1,
                field_type = tlt_field_type(&field_ref.descriptor),
                mangled_name = mangle_field_name(field_class_name, field_name)
            )?;
        }
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
    rettype: &ReturnTypeDescriptor,
    params: &[ParameterDescriptor],
) -> String {
    let mangled_class_name = mangle(class_name);
    let mangled_method_name = match method_name {
        "<init>" => "_init".to_owned(),
        "<clinit>" => "_clinit".to_owned(),
        _ => mangle(method_name),
    };
    let mut mangled = format!("_Jm_{}_{}", mangled_class_name, mangled_method_name);
    mangled.push_str("__");
    match rettype {
        ReturnTypeDescriptor::Void => mangled.push_str("Z"),
        ReturnTypeDescriptor::Field(field_type) => {
            mangled.push_str(&mangle(&field_type.to_string()))
        }
    };
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
