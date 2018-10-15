use super::*;

pub(crate) fn dump_basic_block(idx: u32, block: &BasicBlock, consts: &ConstantPool) {
    println!("╒══[ Basic Block {} ]══", idx);
    println!("│ Stack: {:?}", block.incoming.stack);
    println!("│ Local: {:?}", block.incoming.locals);
    println!("├───────────────────");

    for stmt in block.statements.iter() {
        dump_statement(stmt, consts);
    }

    println!("├───────────────────");
    println!("│ Branch: {:?}", block.branch_stub);
    println!("├───────────────────");
    println!("│ Stack: {:?}", block.outgoing.stack);
    println!("│ Local: {:?}", block.outgoing.locals);
    println!("└───────────────────")
}

fn dump_statement(stmt: &Statement, consts: &ConstantPool) {
    if let Some(ref var) = stmt.assign {
        print!("│ v{}: {:?} ← ", var.1, var.0);
    } else {
        print!("│ _ ← ");
    }
    match stmt.expression {
        Expr::ConstInt(n) => println!("const int {}", n),
        Expr::Invoke(ref target, index, ref args) => {
            let method_ref = consts.get_method_ref(index).unwrap();
            let method_name = consts.get_utf8(method_ref.name_index).unwrap();
            let class = consts.get_class(method_ref.class_index).unwrap();
            let class_name = consts.get_utf8(class.name_index).unwrap();
            match target {
                InvokeTarget::Special(var) => println!(
                    "invoke special({:?}) {}.{}{:?}",
                    var,
                    class_name.replace("/", "."),
                    method_name,
                    args
                ),
                InvokeTarget::Static => println!(
                    "invoke static {}.{}{:?}",
                    class_name.replace("/", "."),
                    method_name,
                    args
                ),
                InvokeTarget::Virtual(var) => println!(
                    "invoke virtual({:?}) {}.{}{:?}",
                    var,
                    class_name.replace("/", "."),
                    method_name,
                    args
                ),
            }
        }
        _ => println!("{:?}", stmt.expression),
    }
}
