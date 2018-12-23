use std::fmt;

use classfile::constant_pool::Constant;
use classfile::descriptors::ReturnTypeDescriptor;
use classfile::instructions::{Disassembler, Instr, LookupSwitch, TableSwitch};
use classfile::{ConstantIndex, ConstantPool};
use failure::{bail, Fallible};
use strbuf::StrBuf;

use crate::blocks::BlockGraph;
use crate::disasm::{InstructionBlock, InstructionBlockMap, InstructionWithRange};
use crate::frame::StackAndLocals;
use crate::types::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockId(u32);

impl BlockId {
    pub fn start() -> Self {
        BlockId(0)
    }

    pub fn from_addr(addr: u32) -> Self {
        BlockId(addr)
    }

    pub fn from_addr_with_offset(addr: u32, offset: i32) -> Self {
        BlockId((addr as i64 + offset as i64) as u32)
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

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
pub(crate) enum Const {
    Int(i32),
    Long(i64),
    String(ConstantIndex),
}

#[derive(Debug)]
pub(crate) enum Expr {
    Var(VarId),
    Const(Const),
    GetStatic(ConstantIndex),
    Invoke(InvokeExpr),
    IInc(VarId, i32),
    New(StrBuf),
    LCmp(VarId, VarId),
    LAdd(VarId, VarId),
}

#[derive(Debug)]
pub(crate) struct ExceptionHandlers; // TODO

#[derive(Debug)]
pub(crate) enum IComparator {
    Lt,
    Le,
    Eq,
    Ge,
}

#[derive(Debug)]
pub(crate) enum AComparator {
    Eq,
    Ne,
}

#[derive(Debug)]
pub(crate) struct Switch {
    pub value: VarId,
    pub default: BlockId,
    pub cases: Vec<(i32, BlockId)>,
}

#[derive(Debug)]
pub(crate) enum BranchStub {
    Goto(BlockId),
    IfICmp(IComparator, VarId, Option<VarId>, BlockId, BlockId),
    IfACmp(AComparator, VarId, VarId, BlockId, BlockId),
    Return(Option<VarId>),
    Switch(Switch),
    Throw(VarId),
}

#[derive(Debug)]
pub(crate) struct Statement {
    pub assign: Option<VarId>,
    pub expression: Expr,
}

#[derive(Debug)]
pub(crate) struct BasicBlock {
    pub address: BlockId,
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

    fn duplicate(&mut self) {
        let var = self.state.pop();
        self.state.push(var.clone());
        self.state.push(var);
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

    fn load_const(self, idx: u16) -> Fallible<Option<TranslateNext>> {
        match self.consts.get_info(ConstantIndex::from_u16(idx)).unwrap() {
            Constant::String(ref string_const) => {
                let var = self.var_id_gen.gen(Type::string());
                self.state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::Const(Const::String(string_const.string_index)),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Constant::Integer(ref integer_const) => {
                let var = self.var_id_gen.gen(Type::Integer);
                self.state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::Const(Const::Int(integer_const.value)),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Constant::Long(ref long_const) => {
                let var = self.var_id_gen.gen(Type::Long);
                self.state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::Const(Const::Long(long_const.value)),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            constant => bail!("unsupported load of constant {:?}", constant),
        }
    }

    fn iconst(self, int: i32) -> Fallible<Option<TranslateNext>> {
        let var = self.var_id_gen.gen(Type::Integer);
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::Const(Const::Int(int)),
        };
        Ok(Some(TranslateNext::Statement(statement)))
    }

    fn lconst(self, int: i64) -> Fallible<Option<TranslateNext>> {
        let var = self.var_id_gen.gen(Type::Long);
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::Const(Const::Long(int)),
        };
        Ok(Some(TranslateNext::Statement(statement)))
    }

    fn lcmp(self) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let var = self.var_id_gen.gen(Type::Integer);
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::LCmp(value1, value2),
        };
        Ok(Some(TranslateNext::Statement(statement)))
    }

    fn ladd(self) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let var = self.var_id_gen.gen(Type::Long);
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::LAdd(value1, value2),
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

    fn athrow(self) -> Fallible<Option<TranslateNext>> {
        let var = self.state.pop();
        return Ok(Some(TranslateNext::Branch(BranchStub::Throw(var), None)));
    }

    fn goto(self, offset: i16) -> Fallible<Option<TranslateNext>> {
        let addr = BlockId::from_addr_with_offset(self.range.start, offset as i32);
        return Ok(Some(TranslateNext::Branch(BranchStub::Goto(addr), None)));
    }

    fn ret(self, with_value: bool) -> Fallible<Option<TranslateNext>> {
        let mut var_opt = None;
        if with_value {
            var_opt = Some(self.state.pop());
        }
        return Ok(Some(TranslateNext::Branch(
            BranchStub::Return(var_opt),
            Some(ExceptionHandlers),
        )));
    }

    fn if_icmp(self, offset: i16, comp: IComparator) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, offset as i32);
        let else_addr = BlockId::from_addr(self.range.end);
        return Ok(Some(TranslateNext::Branch(
            BranchStub::IfICmp(comp, value1, Some(value2), if_addr, else_addr),
            None,
        )));
    }

    fn if_zcmp(self, offset: i16, comp: IComparator) -> Fallible<Option<TranslateNext>> {
        let var = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, offset as i32);
        let else_addr = BlockId::from_addr(self.range.end);
        return Ok(Some(TranslateNext::Branch(
            BranchStub::IfICmp(comp, var, None, if_addr, else_addr),
            None,
        )));
    }

    fn if_acmp(self, offset: i16, comp: AComparator) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, offset as i32);
        let else_addr = BlockId::from_addr(self.range.end);
        return Ok(Some(TranslateNext::Branch(
            BranchStub::IfACmp(comp, value1, value2, if_addr, else_addr),
            None,
        )));
    }

    fn new(self, idx: u16) -> Fallible<Option<TranslateNext>> {
        let class = self.consts.get_class(ConstantIndex::from_u16(idx)).unwrap();
        let class_name = self.consts.get_utf8(class.name_index).unwrap();
        let var = self.var_id_gen.gen(Type::Object(class_name.clone()));
        self.state.push(var.clone());
        let statement = Statement {
            assign: Some(var),
            expression: Expr::New(class_name.clone()),
        };
        return Ok(Some(TranslateNext::Statement(statement)));
    }

    fn table_switch(self, table: &TableSwitch) -> Fallible<Option<TranslateNext>> {
        let value = self.state.pop();
        let default = BlockId::from_addr_with_offset(self.range.start, table.default);
        let mut cases = Vec::with_capacity(table.offsets.len());
        for (idx, offset) in table.offsets.iter().enumerate() {
            let compare_value = table.low + idx as i32;
            let addr = BlockId::from_addr_with_offset(self.range.start, *offset);
            cases.push((compare_value, addr));
        }
        return Ok(Some(TranslateNext::Branch(
            BranchStub::Switch(Switch {
                value,
                default,
                cases,
            }),
            None,
        )));
    }

    fn lookup_switch(self, lookup: &LookupSwitch) -> Fallible<Option<TranslateNext>> {
        let value = self.state.pop();
        let default = BlockId::from_addr_with_offset(self.range.start, lookup.default);
        let mut cases = Vec::with_capacity(lookup.pairs.len());
        for (compare_value, offset) in lookup.pairs.iter() {
            let addr = BlockId::from_addr_with_offset(self.range.start, *offset);
            cases.push((*compare_value, addr));
        }
        return Ok(Some(TranslateNext::Branch(
            BranchStub::Switch(Switch {
                value,
                default,
                cases,
            }),
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
            Instr::ALoad2 => t.load(2),
            Instr::AStore1 => t.store(1),
            Instr::AStore2 => t.store(2),
            Instr::ILoad(idx) => t.load(*idx as usize),
            Instr::IStore(idx) => t.store(*idx as usize),
            Instr::LLoad(idx) => t.load(*idx as usize),
            Instr::LStore(idx) => t.store(*idx as usize),
            Instr::Dup => t.duplicate(),
            Instr::IConst0 => return t.iconst(0),
            Instr::IConst1 => return t.iconst(1),
            Instr::IConst2 => return t.iconst(2),
            Instr::IConst3 => return t.iconst(3),
            Instr::LConst0 => return t.lconst(0),
            Instr::LConst1 => return t.lconst(1),
            Instr::BiPush(b) => return t.iconst(*b as i32),
            Instr::IInc(idx, int) => return t.iinc(*idx, *int as i32),
            Instr::GetStatic(idx) => return t.get_static(*idx),
            Instr::LdC(idx) => return t.load_const(*idx as u16),
            Instr::LdCW(idx) => return t.load_const(*idx),
            Instr::LdC2W(idx) => return t.load_const(*idx),
            Instr::InvokeSpecial(idx) => return t.invoke(InvokeType::Special, *idx),
            Instr::InvokeStatic(idx) => return t.invoke(InvokeType::Static, *idx),
            Instr::InvokeVirtual(idx) => return t.invoke(InvokeType::Virtual, *idx),
            Instr::Goto(offset) => return t.goto(*offset),
            Instr::Return => return t.ret(false),
            Instr::IReturn => return t.ret(true),
            Instr::AReturn => return t.ret(true),
            Instr::IfLt(offset) => return t.if_zcmp(*offset, IComparator::Lt),
            Instr::IfLe(offset) => return t.if_zcmp(*offset, IComparator::Le),
            Instr::IfEq(offset) => return t.if_zcmp(*offset, IComparator::Eq),
            Instr::IfGe(offset) => return t.if_zcmp(*offset, IComparator::Ge),
            Instr::IfICmpGe(offset) => return t.if_icmp(*offset, IComparator::Ge),
            Instr::IfICmpLe(offset) => return t.if_icmp(*offset, IComparator::Le),
            Instr::IfACmpEq(offset) => return t.if_acmp(*offset, AComparator::Eq),
            Instr::IfACmpNe(offset) => return t.if_acmp(*offset, AComparator::Ne),
            Instr::New(idx) => return t.new(*idx),
            Instr::TableSwitch(table) => return t.table_switch(table),
            Instr::LookupSwitch(lookup) => return t.lookup_switch(lookup),
            Instr::LCmp => return t.lcmp(),
            Instr::LAdd => return t.ladd(),
            Instr::AThrow => return t.athrow(),
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
    let address = BlockId(instr_block.range.start);
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
                let branch_stub = BranchStub::Goto(BlockId(instr_block.range.end));
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
    let mut remaining = vec![(BlockId::start(), incoming)];
    while let Some((addr, state)) = remaining.pop() {
        if !blocks.contains(addr) {
            let instr_block = instr_block_map.block_starting_at(addr.0);
            let block = translate_block(instr_block, state, &consts, var_id_gen)?;
            match block.branch_stub {
                BranchStub::Goto(addr) => {
                    remaining.push((addr, block.outgoing.new_with_same_shape(var_id_gen)));
                }
                BranchStub::IfICmp(_, _, _, if_addr, else_addr)
                | BranchStub::IfACmp(_, _, _, if_addr, else_addr) => {
                    remaining.push((if_addr, block.outgoing.new_with_same_shape(var_id_gen)));
                    remaining.push((else_addr, block.outgoing.new_with_same_shape(var_id_gen)));
                }
                BranchStub::Switch(ref switch) => {
                    remaining.push((
                        switch.default,
                        block.outgoing.new_with_same_shape(var_id_gen),
                    ));
                    for (_, addr) in switch.cases.iter() {
                        remaining.push((*addr, block.outgoing.new_with_same_shape(var_id_gen)));
                    }
                }
                BranchStub::Throw(_) => {}
                BranchStub::Return(_) => {}
            }
            blocks.insert(block);
        }
    }
    blocks.calculate_edges();
    Ok(blocks)
}
