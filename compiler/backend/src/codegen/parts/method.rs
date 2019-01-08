use std::fmt::Write;
use std::sync::Arc;

use classfile::{ClassFile, ConstantPool, Method};
use failure::Fallible;

use frontend::blocks::BlockGraph;
use frontend::classes::ClassGraph;
use frontend::translate::{BasicBlock, BlockId, BranchStub, Expr, Statement, Switch};

use crate::codegen::common::*;
use crate::codegen::decls::DeclDatabase;
use crate::codegen::Target;
use crate::layout::{FieldLayoutMap, VTableMap};
use crate::mangle;

use super::expr::ExprCodeGen;

pub struct MethodCodeGen<'a> {
    pub out: &'a mut String,
    pub decls: &'a mut DeclDatabase,
    pub class: &'a Arc<ClassFile>,
    pub classes: &'a ClassGraph,
    pub vtables: &'a VTableMap,
    pub field_layouts: &'a FieldLayoutMap,
    pub var_id_gen: &'a mut TmpVarIdGen,
    pub target: &'a Arc<Target>,
}

impl<'a> MethodCodeGen<'a> {
    pub fn gen_method(
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
            write!(self.out, "{} {}", tlt_type(&var.get_type()), OpVal(var))?;
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
            BranchStub::Return(ret_opt) => {
                if let Some(ret) = ret_opt {
                    writeln!(
                        self.out,
                        "  ret {} {}",
                        tlt_type(&ret.get_type()),
                        OpVal(ret)
                    )?;
                } else {
                    writeln!(self.out, "  ret void")?;
                }
            }
            BranchStub::Switch(switch) => self.gen_switch(switch)?,
            BranchStub::Throw(var) => {
                writeln!(
                    self.out,
                    "  call void @_Jrt_throw(%ref {}) noreturn",
                    OpVal(var)
                )?;
                writeln!(self.out, "  unreachable")?;
            }
        }
        Ok(())
    }

    fn gen_switch(&mut self, switch: &Switch) -> Fallible<()> {
        write!(
            self.out,
            "  switch i32 {}, label %B{} [",
            OpVal(&switch.value),
            switch.default
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
            dest = Dest::Assign(DestAssign::Var(var.clone()));
        } else {
            dest = Dest::Ignore;
        }
        self.gen_expr(&stmt.expression, consts, dest)
    }

    fn gen_expr(&mut self, expr: &Expr, consts: &ConstantPool, dest: Dest) -> Fallible<()> {
        let mut expr_code_gen = ExprCodeGen {
            out: self.out,
            decls: self.decls,
            class: self.class,
            classes: self.classes,
            vtables: self.vtables,
            field_layouts: self.field_layouts,
            var_id_gen: self.var_id_gen,
            target: self.target,
        };
        expr_code_gen.gen_expr(expr, consts, dest)
    }

    fn gen_phi_nodes(&mut self, block: &BasicBlock, blocks: &BlockGraph) -> Fallible<()> {
        let phis = blocks.phis(block);
        for (var, bindings) in phis.iter() {
            write!(self.out, "  %v{} = phi {} ", var.1, tlt_type(&var.0))?;
            for (i, (out_var, addr)) in bindings.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ")?;
                }
                write!(self.out, "[ {}, %B{} ]", OpVal(out_var), addr)?;
            }
            writeln!(self.out)?;
        }
        Ok(())
    }
}
