use std::collections::BTreeMap;

use classfile::attrs::stack_map_table::VerificationTypeInfo;
use classfile::descriptors::{BaseType, FieldType, ParameterDescriptor, ReturnTypeDescriptor};
use classfile::{constant_pool::Constant, Method};

use super::*;
use graph::BlockGraph;

pub(crate) fn gen_prelude(class: &ClassFile) {
    for index in class.constant_pool.indices() {
        match class.constant_pool.get_info(index).unwrap() {
            Constant::MethodRef(_) => {
                let method_ref = class.constant_pool.get_method_ref(index).unwrap();
                let method_name = class.constant_pool.get_utf8(method_ref.name_index).unwrap();
                let method_class = class
                    .constant_pool
                    .get_class(method_ref.class_index)
                    .unwrap();
                let method_class_name = class
                    .constant_pool
                    .get_utf8(method_class.name_index)
                    .unwrap();
                print!(
                    "declare {return_type} @{mangled_name}(",
                    return_type = tlt_return_type(&method_ref.descriptor.ret),
                    mangled_name =
                        mangle_method_name(method_class_name, method_name, &class.constant_pool)
                );
                print!("i8*");
                for ParameterDescriptor::Field(field) in method_ref.descriptor.params.iter() {
                    print!(", {}", tlt_field_type(field));
                }
                println!(")");
            }
            Constant::FieldRef(_) => {
                let field_ref = class.constant_pool.get_field_ref(index).unwrap();
                let field_name = class.constant_pool.get_utf8(field_ref.name_index).unwrap();
                let field_class = class
                    .constant_pool
                    .get_class(field_ref.class_index)
                    .unwrap();
                let field_class_name = class
                    .constant_pool
                    .get_utf8(field_class.name_index)
                    .unwrap();
                println!(
                    "declare {field_type} @{mangled_name}__get(i8*)",
                    field_type = tlt_field_type(&field_ref.descriptor),
                    mangled_name = mangle_field_name(field_class_name, field_name)
                );
                println!(
                    "declare void @{mangled_name}__set(i8*, {field_type})",
                    field_type = tlt_field_type(&field_ref.descriptor),
                    mangled_name = mangle_field_name(field_class_name, field_name)
                );
            }
            _ => {}
        }
    }
}

pub(crate) fn gen_method(
    class: &ClassFile,
    method: &Method,
    blocks: &BlockGraph,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) {
    let class_name = consts.get_utf8(class.get_this_class().name_index).unwrap();
    let method_name = consts.get_utf8(method.name_index).unwrap();
    print!(
        "\ndefine {return_type} @{mangled_name}(",
        return_type = tlt_return_type(&method.descriptor.ret),
        mangled_name = mangle_method_name(class_name, method_name, consts)
    );
    for (i, (_, var)) in blocks.lookup(0).incoming.locals.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("{} %v{}", tlt_type(&var.0), var.1);
    }
    println!(") {{");
    for block in blocks.blocks() {
        gen_block(block, blocks, &class.constant_pool, var_id_gen);
    }
    println!("}}");
}

fn gen_block(
    block: &BasicBlock,
    blocks: &BlockGraph,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) {
    println!("B{}:", block.address);
    gen_phi_nodes(block, blocks);
    for stmt in block.statements.iter() {
        gen_statement(stmt, consts);
    }
    match &block.branch_stub {
        BranchStub::Goto(addr) => println!("  br label %B{}", addr),
        BranchStub::Return(None) => println!("  ret void"),
        BranchStub::IfEq(var, if_addr, else_addr) => {
            let tmp = var_id_gen.gen(Type::int());
            println!("  %tmp{} = icmp eq i32 0, %v{}", tmp.1, var.1);
            println!(
                "  br i1 %tmp{}, label %B{}, label %B{}",
                tmp.1, if_addr, else_addr
            );
        }
        _ => unimplemented!(),
    }
}

fn gen_statement(stmt: &Statement, consts: &ConstantPool) {
    if let Some(ref var) = stmt.assign {
        print!("  %v{} = ", var.1);
    } else {
        print!("  ");
    }
    gen_expr(&stmt.expression, consts)
}

fn gen_expr(expr: &Expr, consts: &ConstantPool) {
    match expr {
        Expr::ConstInt(i) => println!("and i32 {}, {}", i, i),
        Expr::GetStatic(index) => gen_expr_get_static(*index, consts),
        Expr::Invoke(subexpr) => gen_expr_invoke(subexpr, consts),
        _ => println!("select i1 true, i8* undef, i8* undef; {:?}", expr),
    }
}

fn gen_expr_invoke(expr: &InvokeExpr, consts: &ConstantPool) {
    let method_ref = consts.get_method_ref(expr.index).unwrap();
    let method_name = consts.get_utf8(method_ref.name_index).unwrap();
    let method_class = consts.get_class(method_ref.class_index).unwrap();
    let method_class_name = consts.get_utf8(method_class.name_index).unwrap();
    print!(
        "call {return_type} @{mangled_name}(",
        return_type = tlt_return_type(&method_ref.descriptor.ret),
        mangled_name = mangle_method_name(method_class_name, method_name, consts)
    );
    match expr.target {
        InvokeTarget::Static => print!("i8* null"),
        InvokeTarget::Special(ref var) => print!("i8* %v{}", var.1),
        InvokeTarget::Virtual(ref var) => print!("i8* %v{}", var.1),
    }
    for var in expr.args.iter() {
        print!(", {} %v{}", tlt_type(&var.0), var.1);
    }
    println!(")");
}

fn gen_expr_get_static(index: ConstantIndex, consts: &ConstantPool) {
    let field_ref = consts.get_field_ref(index).unwrap();
    let field_name = consts.get_utf8(field_ref.name_index).unwrap();
    let field_class = consts.get_class(field_ref.class_index).unwrap();
    let field_class_name = consts.get_utf8(field_class.name_index).unwrap();
    println!(
        "call {field_type} @{mangled_name}__get(i8* null)",
        field_type = tlt_field_type(&field_ref.descriptor),
        mangled_name = mangle_field_name(field_class_name, field_name)
    );
}

fn gen_phi_nodes(block: &BasicBlock, blocks: &BlockGraph) {
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
        print!("  %v{} = phi {} ", var.1, tlt_type(&var.0));
        for (i, (out_var, addr)) in bindings.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("[ %v{}, %B{} ]", out_var.1, addr);
        }
        println!("");
    }
}

fn mangle_field_name(class_name: &str, field_name: &str) -> String {
    format!("_Jf_{}_{}", class_name.replace("/", "_"), field_name)
}

fn mangle_method_name(class_name: &str, mut method_name: &str, consts: &ConstantPool) -> String {
    if method_name == "<init>" {
        method_name = "_init";
    }
    format!("_Jm_{}_{}", class_name.replace("/", "_"), method_name)
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
        FieldType::Object(_) => "i8*",
        FieldType::Array(_) => "i8*",
    }
}

fn tlt_type(t: &Type) -> &'static str {
    match t.info {
        VerificationTypeInfo::Integer => "i32",
        VerificationTypeInfo::Long => "i64",
        VerificationTypeInfo::Float => "float",
        VerificationTypeInfo::Double => "double",
        VerificationTypeInfo::Null => "i8*",
        VerificationTypeInfo::Object(_) => "i8*",
        VerificationTypeInfo::UninitializedThis => "i8*",
        _ => unimplemented!(),
    }
}
