use std::fmt;

use classfile::constant_pool::Constant;
use classfile::descriptors::ReturnTypeDescriptor;
use classfile::instructions::{Disassembler, Instr};
use classfile::{ConstantIndex, ConstantPool};
use failure::Fallible;

use blocks::BlockGraph;
use disasm::{InstructionBlock, InstructionBlockMap, InstructionWithRange};
use frame::StackAndLocals;
use types::Type;

#[derive(Debug)]
struct BlockId(usize);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarId(pub Type, pub u64);

impl fmt::Debug for VarId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "v{}", self.1)
    }
}

pub struct VarIdGen {
    next_id: u64,
}

impl VarIdGen {
    pub fn new() -> Self {
        VarIdGen { next_id: 0 }
    }

    pub fn gen(&mut self, var_type: Type) -> VarId {
        let var_id = VarId(var_type, self.next_id);
        self.next_id += 1;
        var_id
    }
}

#[derive(Debug)]
enum InvokeType {
    Static,
    Special,
    Virtual,
}

#[derive(Debug)]
pub(crate) enum InvokeTarget {
    Static,
    Special(VarId),
    Virtual(VarId),
}

#[derive(Debug)]
pub(crate) struct InvokeExpr {
    pub target: InvokeTarget,
    pub index: ConstantIndex,
    pub args: Vec<VarId>,
}

#[derive(Debug)]
pub(crate) enum Expr {
    Var(VarId),
    Const(ConstantIndex),
    ConstInt(i32),
    ConstString(ConstantIndex),
    GetStatic(ConstantIndex),
    Invoke(InvokeExpr),
    IInc(VarId, i32),
}

#[derive(Debug)]
pub(crate) struct ExceptionHandlers; // TODO

#[derive(Debug)]
pub(crate) enum Comparator {
    Eq,
    Ge,
}

#[derive(Debug)]
pub(crate) enum BranchStub {
    Goto(u32),
    IfICmp(Comparator, VarId, Option<VarId>, u32, u32),
    Return(Option<VarId>),
    Invoke(Option<VarId>, InvokeExpr, u32),
}

#[derive(Debug)]
enum Branch {
    Return(Option<VarId>),
    IfEq(VarId, BlockId, BlockId),
}

#[derive(Debug)]
pub(crate) struct Statement {
    pub assign: Option<VarId>,
    pub expression: Expr,
}

#[derive(Debug)]
pub(crate) struct BasicBlock {
    pub address: u32,
    pub incoming: StackAndLocals,
    pub statements: Vec<Statement>,
    pub branch_stub: BranchStub,
    pub exceptions: Option<ExceptionHandlers>,
    pub outgoing: StackAndLocals,
}

enum TranslateNext {
    Statement(Statement),
    Branch(BranchStub, Option<ExceptionHandlers>),
}

struct TranslateInstr<'a> {
    range: &'a std::ops::Range<u32>,
    state: &'a mut StackAndLocals,
    consts: &'a ConstantPool,
    var_id_gen: &'a mut VarIdGen,
}

impl<'a> TranslateInstr<'a> {
    fn load(&mut self, idx: usize) {
        self.state.load(idx)
    }

    fn store(&mut self, idx: usize) {
        self.state.store(idx)
    }

    fn get_static(self, idx: u16) -> Fallible<Option<TranslateNext>> {
        let field = self
            .consts
            .get_field_ref(ConstantIndex::from_u16(idx))
            .unwrap();
        let var = self.var_id_gen.gen(Type::from_field_type(field.descriptor));
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::GetStatic(ConstantIndex::from_u16(idx)),
        };
        return Ok(Some(TranslateNext::Statement(statement)));
    }

    fn load_const(self, idx: u8) -> Fallible<Option<TranslateNext>> {
        match self.consts.get_info(ConstantIndex::from_u8(idx)).unwrap() {
            Constant::String(ref string_const) => {
                let var = self.var_id_gen.gen(Type::string());
                self.state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::ConstString(string_const.string_index),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            constant => bail!("unsupported load of constant {:?}", constant),
        }
    }

    fn iconst(self, int: i32) -> Fallible<Option<TranslateNext>> {
        let var = self.var_id_gen.gen(Type::int());
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::ConstInt(int),
        };
        Ok(Some(TranslateNext::Statement(statement)))
    }

    fn iinc(self, idx: u8, int: i32) -> Fallible<Option<TranslateNext>> {
        let var2 = self.var_id_gen.gen(Type::int());
        let var1 = self.state.locals[&(idx as usize)].clone();
        self.state.locals.insert(idx as usize, var2.clone());
        let statement = Statement {
            assign: Some(var2),
            expression: Expr::IInc(var1, int),
        };
        Ok(Some(TranslateNext::Statement(statement)))
    }

    fn invoke(self, invoke: InvokeType, idx: u16) -> Fallible<Option<TranslateNext>> {
        let cidx = ConstantIndex::from_u16(idx);
        let method = self.consts.get_method_ref(cidx).unwrap();
        let method_args_len = method.descriptor.params.len();
        let args = self.state.pop_n(method_args_len);
        let expr = match invoke {
            InvokeType::Static => InvokeExpr {
                target: InvokeTarget::Static,
                index: cidx,
                args,
            },
            InvokeType::Special => InvokeExpr {
                target: InvokeTarget::Special(self.state.pop()),
                index: cidx,
                args,
            },
            InvokeType::Virtual => InvokeExpr {
                target: InvokeTarget::Virtual(self.state.pop()),
                index: cidx,
                args,
            },
        };
        let return_type = match method.descriptor.ret {
            ReturnTypeDescriptor::Void => None,
            ReturnTypeDescriptor::Field(field_type) => Some(Type::from_field_type(field_type)),
        };
        let return_var = return_type.map(|t| self.var_id_gen.gen(t));
        if let Some(ref var) = return_var {
            self.state.push(var.clone());
        }
        let statement = Statement {
            assign: return_var,
            expression: Expr::Invoke(expr),
        };
        return Ok(Some(TranslateNext::Statement(statement)));
    }

    fn goto(self, offset: i16) -> Fallible<Option<TranslateNext>> {
        let addr = (self.range.start as i64 + offset as i64) as u32;
        return Ok(Some(TranslateNext::Branch(BranchStub::Goto(addr), None)));
    }

    fn ret(self) -> Fallible<Option<TranslateNext>> {
        return Ok(Some(TranslateNext::Branch(
            BranchStub::Return(None),
            Some(ExceptionHandlers),
        )));
    }

    fn if_icmp(self, offset: i16, comp: Comparator) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let if_addr = (self.range.start as i64 + offset as i64) as u32;
        let else_addr = self.range.end;
        return Ok(Some(TranslateNext::Branch(
            BranchStub::IfICmp(comp, value1, Some(value2), if_addr, else_addr),
            None,
        )));
    }

    fn if_zcmp(self, offset: i16, comp: Comparator) -> Fallible<Option<TranslateNext>> {
        let var = self.state.pop();
        let if_addr = (self.range.start as i64 + offset as i64) as u32;
        let else_addr = self.range.end;
        return Ok(Some(TranslateNext::Branch(
            BranchStub::IfICmp(comp, var, None, if_addr, else_addr),
            None,
        )));
    }
}

fn translate_next(
    instrs: &mut Iterator<Item = &InstructionWithRange>,
    state: &mut StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<Option<TranslateNext>> {
    for InstructionWithRange { range, instr } in instrs {
        let mut t = TranslateInstr {
            range,
            state,
            consts,
            var_id_gen,
        };
        match instr {
            Instr::ALoad0 => t.load(0),
            Instr::ALoad1 => t.load(1),
            Instr::AStore1 => t.store(1),
            Instr::ILoad(idx) => t.load(*idx as usize),
            Instr::IStore(idx) => t.store(*idx as usize),
            Instr::IConst0 => return t.iconst(0),
            Instr::IConst1 => return t.iconst(1),
            Instr::IConst2 => return t.iconst(2),
            Instr::IConst3 => return t.iconst(3),
            Instr::IInc(idx, int) => return t.iinc(*idx, *int as i32),
            Instr::GetStatic(idx) => return t.get_static(*idx),
            Instr::LdC(idx) => return t.load_const(*idx),
            Instr::InvokeSpecial(idx) => return t.invoke(InvokeType::Special, *idx),
            Instr::InvokeStatic(idx) => return t.invoke(InvokeType::Static, *idx),
            Instr::InvokeVirtual(idx) => return t.invoke(InvokeType::Virtual, *idx),
            Instr::Goto(offset) => return t.goto(*offset),
            Instr::Return => return t.ret(),
            Instr::IfEq(offset) => return t.if_zcmp(*offset, Comparator::Eq),
            Instr::IfICmpGe(offset) => return t.if_icmp(*offset, Comparator::Ge),
            _ => bail!("unsupported instruction {:?}", instr),
        }
    }
    Ok(None)
}

fn translate_block(
    instr_block: &InstructionBlock,
    incoming: StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<BasicBlock> {
    let address = instr_block.range.start;
    let mut state = incoming.clone();
    let mut statements = Vec::new();
    let mut instrs = instr_block.instrs.iter();
    loop {
        match translate_next(&mut instrs, &mut state, &consts, var_id_gen)? {
            Some(TranslateNext::Statement(stmt)) => {
                statements.push(stmt);
            }
            Some(TranslateNext::Branch(branch_stub, exceptions)) => {
                return Ok(BasicBlock {
                    address,
                    incoming,
                    statements,
                    branch_stub,
                    exceptions,
                    outgoing: state,
                });
            }
            None => {
                let branch_stub = BranchStub::Goto(instr_block.range.end);
                return Ok(BasicBlock {
                    address,
                    incoming,
                    statements,
                    branch_stub,
                    exceptions: Some(ExceptionHandlers),
                    outgoing: state,
                });
            }
        }
    }
}

pub(crate) fn translate_method(
    dasm: Disassembler,
    incoming: StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<BlockGraph> {
    let instr_block_map = InstructionBlockMap::build(dasm)?;
    let mut blocks = BlockGraph::new();
    let mut remaining = vec![(0, incoming)];
    while let Some((addr, state)) = remaining.pop() {
        if !blocks.contains(addr) {
            let instr_block = instr_block_map.block_starting_at(addr);
            let block = translate_block(instr_block, state, &consts, var_id_gen)?;
            match block.branch_stub {
                BranchStub::Goto(addr) => {
                    remaining.push((addr, block.outgoing.new_with_same_shape(var_id_gen)));
                }
                BranchStub::IfICmp(_, _, _, if_addr, else_addr) => {
                    remaining.push((if_addr, block.outgoing.new_with_same_shape(var_id_gen)));
                    remaining.push((else_addr, block.outgoing.new_with_same_shape(var_id_gen)));
                }
                BranchStub::Invoke(_, _, addr) => {
                    remaining.push((addr, block.outgoing.new_with_same_shape(var_id_gen)));
                }

                BranchStub::Return(_) => {}
            }
            blocks.insert(block);
        }
    }
    blocks.calculate_edges();
    Ok(blocks)
}
