use std::fmt;

use classfile::constant_pool::Constant;
use classfile::descriptors::ReturnTypeDescriptor;
use classfile::instructions::{Disassembler, Instr, LookupSwitch, TableSwitch};
use classfile::{ConstantIndex, ConstantPool, MethodRef};
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
        BlockId((i64::from(addr) + i64::from(offset)) as u32)
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarId(pub Type, pub u64);

#[derive(Default)]
pub struct VarIdGen {
    next_id: u64,
}

impl VarIdGen {
    pub fn gen(&mut self, var_type: Type) -> VarId {
        let var_id = VarId(var_type, self.next_id);
        self.next_id += 1;
        var_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvokeType {
    Static,
    Special,
    Virtual,
    Interface,
}

#[derive(Debug)]
pub enum InvokeTarget {
    Static,
    Special(Op),
    Virtual(Op),
    Interface(Op),
}

#[derive(Debug)]
pub struct InvokeExpr {
    pub target: InvokeTarget,
    pub method: MethodRef,
    pub args: Vec<Op>,
}

#[derive(Clone, Debug)]
pub enum Const {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Null,
}

impl Const {
    pub fn get_type(&self) -> Type {
        match self {
            Const::Int(_) => Type::Int,
            Const::Long(_) => Type::Long,
            Const::Float(_) => Type::Float,
            Const::Double(_) => Type::Double,
            Const::Null => Type::Reference,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Op {
    Var(VarId),
    Const(Const),
}

impl Op {
    pub fn get_type(&self) -> Type {
        match self {
            Op::Var(v) => v.0.clone(),
            Op::Const(c) => c.get_type(),
        }
    }
}

#[derive(Debug)]
pub enum BinaryOperation {
    Add,
    Sub,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRightLogical,
    ShiftRightArithmetic,
}

#[derive(Debug)]
pub struct BinaryExpr {
    pub operation: BinaryOperation,
    pub result_type: Type,
    pub operand_left: Op,
    pub operand_right: Op,
}

#[derive(Debug)]
pub enum ConvertOperation {
    IntToChar,
    IntToByte,
    IntToShort,
}

#[derive(Debug)]
pub struct ConvertExpr {
    pub operation: ConvertOperation,
    pub operand: Op,
}

#[derive(Debug)]
pub enum NaNCmpMode {
    Greater,
    Less,
}

#[derive(Debug)]
pub enum CompareExpr {
    ICmp(IComparator, Op, Op),
    ACmp(AComparator, Op, Op),
    LCmp(Op, Op),
    FCmp(Op, Op, NaNCmpMode),
    DCmp(Op, Op, NaNCmpMode),
}

#[derive(Debug)]
pub enum MonitorStateTransition {
    Enter,
    Exit,
}

#[derive(Debug)]
pub enum Expr {
    String(ConstantIndex),
    GetStatic(ConstantIndex),
    GetField(Op, ConstantIndex),
    PutField(Op, ConstantIndex, Op),
    Invoke(InvokeExpr),
    New(StrBuf),
    Compare(CompareExpr),
    Binary(BinaryExpr),
    ArrayNew(Type, Op),
    ArrayLength(Op),
    ArrayLoad(Type, Op, Op),
    ArrayStore(Type, Op, Op, Op),
    Convert(ConvertExpr),
    Monitor(Op, MonitorStateTransition),
}

#[derive(Debug)]
pub struct ExceptionHandlers; // TODO

#[derive(Debug)]
pub enum IComparator {
    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,
}

#[derive(Debug)]
pub enum AComparator {
    Eq,
    Ne,
}

#[derive(Debug)]
pub struct Switch {
    pub value: Op,
    pub default: BlockId,
    pub cases: Vec<(i32, BlockId)>,
}

impl Switch {
    fn goto(addr: BlockId) -> Switch {
        Switch {
            value: Op::Const(Const::Int(0)),
            default: addr,
            cases: vec![],
        }
    }

    fn if_else(var: VarId, if_addr: BlockId, else_addr: BlockId) -> Switch {
        Switch {
            value: Op::Var(var),
            default: else_addr,
            cases: vec![(1, if_addr)],
        }
    }
}

#[derive(Debug)]
pub enum BranchStub {
    Return(Option<Op>),
    Switch(Switch),
    Throw(Op),
}

#[derive(Debug)]
pub struct Statement {
    pub assign: Option<VarId>,
    pub expression: Expr,
}

#[derive(Debug)]
pub struct BasicBlock {
    pub address: BlockId,
    pub incoming: StackAndLocals,
    pub statements: Vec<Statement>,
    pub branch_stub: BranchStub,
    pub exceptions: Option<ExceptionHandlers>,
    pub outgoing: StackAndLocals,
}

struct TranslateNext(BranchStub, Option<ExceptionHandlers>);

struct TranslateInstr<'a> {
    range: &'a std::ops::Range<u32>,
    state: &'a mut StackAndLocals,
    consts: &'a ConstantPool,
    var_id_gen: &'a mut VarIdGen,
    stmts: &'a mut Vec<Statement>,
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

    fn duplicate2(&mut self) {
        let var1 = self.state.pop();
        let var2 = self.state.pop();
        self.state.push(var2.clone());
        self.state.push(var1.clone());
        self.state.push(var2);
        self.state.push(var1);
    }

    fn pop(&mut self, n: usize) {
        self.state.pop_n(n);
    }

    fn push_const(&mut self, c: Const) {
        self.state.push(Op::Const(c));
    }

    fn get_static(&mut self, idx: u16) {
        let field = self
            .consts
            .get_field_ref(ConstantIndex::from_u16(idx))
            .unwrap();
        let var = self
            .var_id_gen
            .gen(Type::from_field_type(&field.descriptor));
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::GetStatic(ConstantIndex::from_u16(idx)),
        };
        self.stmts.push(statement);
    }

    fn get_field(&mut self, idx: u16) {
        let object = self.state.pop();
        let field = self
            .consts
            .get_field_ref(ConstantIndex::from_u16(idx))
            .unwrap();
        let var = self
            .var_id_gen
            .gen(Type::from_field_type(&field.descriptor));
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::GetField(object, ConstantIndex::from_u16(idx)),
        };
        self.stmts.push(statement);
    }

    fn put_field(&mut self, idx: u16) {
        let value = self.state.pop();
        let object = self.state.pop();
        let field = self
            .consts
            .get_field_ref(ConstantIndex::from_u16(idx))
            .unwrap();
        let var = self
            .var_id_gen
            .gen(Type::from_field_type(&field.descriptor));
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::PutField(object, ConstantIndex::from_u16(idx), value),
        };
        self.stmts.push(statement);
    }

    fn load_const(&mut self, idx: u16) {
        match self.consts.get_info(ConstantIndex::from_u16(idx)).unwrap() {
            Constant::String(ref string_const) => {
                let var = self.var_id_gen.gen(Type::Reference);
                self.state.push(Op::Var(var.clone()));
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::String(string_const.string_index),
                };
                self.stmts.push(statement);
            }
            Constant::Integer(ref integer_const) => {
                self.state.push(Op::Const(Const::Int(integer_const.value)));
            }
            Constant::Long(ref long_const) => {
                self.state.push(Op::Const(Const::Long(long_const.value)));
            }
            Constant::Float(ref float_const) => {
                self.state.push(Op::Const(Const::Float(float_const.value)));
            }
            Constant::Double(ref double_const) => {
                self.state
                    .push(Op::Const(Const::Double(double_const.value)));
            }
            constant => panic!("unsupported load of constant {:?}", constant),
        }
    }

    fn lcmp(&mut self) {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let var = self.var_id_gen.gen(Type::Int);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::Compare(CompareExpr::LCmp(value1, value2)),
        };
        self.stmts.push(statement);
    }

    fn fcmp(&mut self, mode: NaNCmpMode) {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let var = self.var_id_gen.gen(Type::Int);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::Compare(CompareExpr::FCmp(value1, value2, mode)),
        };
        self.stmts.push(statement);
    }

    fn dcmp(&mut self, mode: NaNCmpMode) {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let var = self.var_id_gen.gen(Type::Int);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::Compare(CompareExpr::DCmp(value1, value2, mode)),
        };
        self.stmts.push(statement);
    }

    fn binary(&mut self, result_type: Type, operation: BinaryOperation) {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let result = self.var_id_gen.gen(result_type.clone());
        self.state.push(Op::Var(result.clone()));
        let binary_expr = BinaryExpr {
            operation,
            result_type,
            operand_left: value1,
            operand_right: value2,
        };
        let statement = Statement {
            assign: Some(result),
            expression: Expr::Binary(binary_expr),
        };
        self.stmts.push(statement);
    }

    fn iinc(&mut self, idx: u8, int: i32) {
        let var2 = self.var_id_gen.gen(Type::Int);
        let var1 = self.state.locals[&(idx as usize)].clone();
        self.state
            .locals
            .insert(idx as usize, Op::Var(var2.clone()));
        let binary_expr = BinaryExpr {
            operation: BinaryOperation::Add,
            result_type: Type::Int,
            operand_left: var1,
            operand_right: Op::Const(Const::Int(int)),
        };
        let statement = Statement {
            assign: Some(var2),
            expression: Expr::Binary(binary_expr),
        };
        self.stmts.push(statement);
    }

    fn invoke(&mut self, invoke: InvokeType, idx: u16) {
        let cidx = ConstantIndex::from_u16(idx);
        let method = if invoke == InvokeType::Interface {
            self.consts.get_interface_method_ref(cidx).unwrap()
        } else {
            self.consts.get_method_ref(cidx).unwrap()
        };
        let method_args_len = method.descriptor.params.len();
        let args = self.state.pop_n(method_args_len);
        let return_type = match method.descriptor.ret {
            ReturnTypeDescriptor::Void => None,
            ReturnTypeDescriptor::Field(ref field_type) => Some(Type::from_field_type(&field_type)),
        };
        let expr = match invoke {
            InvokeType::Static => InvokeExpr {
                target: InvokeTarget::Static,
                method,
                args,
            },
            InvokeType::Special => InvokeExpr {
                target: InvokeTarget::Special(self.state.pop()),
                method,
                args,
            },
            InvokeType::Virtual => InvokeExpr {
                target: InvokeTarget::Virtual(self.state.pop()),
                method,
                args,
            },
            InvokeType::Interface => InvokeExpr {
                target: InvokeTarget::Interface(self.state.pop()),
                method,
                args,
            },
        };
        let return_var = return_type.map(|t| self.var_id_gen.gen(t));
        if let Some(ref var) = return_var {
            self.state.push(Op::Var(var.clone()));
        }
        let statement = Statement {
            assign: return_var,
            expression: Expr::Invoke(expr),
        };
        self.stmts.push(statement);
    }

    fn array_new(&mut self, component_type: Type) {
        let count = self.state.pop();
        let var = self.var_id_gen.gen(Type::Reference);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::ArrayNew(component_type, count),
        };
        self.stmts.push(statement);
    }

    fn array_length(&mut self) {
        let arrayref = self.state.pop();
        let var = self.var_id_gen.gen(Type::Int);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::ArrayLength(arrayref),
        };
        self.stmts.push(statement);
    }

    fn array_load(&mut self, component_type: Type) {
        let index = self.state.pop();
        let arrayref = self.state.pop();
        let var = self.var_id_gen.gen(component_type.clone());
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::ArrayLoad(component_type, arrayref, index),
        };
        self.stmts.push(statement);
    }

    fn array_store(&mut self, component_type: Type) {
        let value = self.state.pop();
        let index = self.state.pop();
        let arrayref = self.state.pop();
        let statement = Statement {
            assign: None,
            expression: Expr::ArrayStore(component_type, arrayref, index, value),
        };
        self.stmts.push(statement);
    }

    fn athrow(self) -> Fallible<Option<TranslateNext>> {
        let var = self.state.pop();
        Ok(Some(TranslateNext(BranchStub::Throw(var), None)))
    }

    fn goto(self, offset: i16) -> Fallible<Option<TranslateNext>> {
        let addr = BlockId::from_addr_with_offset(self.range.start, i32::from(offset));
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch::goto(addr)),
            None,
        )))
    }

    fn ret(self, with_value: bool) -> Fallible<Option<TranslateNext>> {
        let var_opt = if with_value {
            Some(self.state.pop())
        } else {
            None
        };
        Ok(Some(TranslateNext(
            BranchStub::Return(var_opt),
            Some(ExceptionHandlers),
        )))
    }

    fn if_icmp(self, offset: i16, comp: IComparator) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, i32::from(offset));
        let else_addr = BlockId::from_addr(self.range.end);
        let tmpvar = self.var_id_gen.gen(Type::Boolean);
        let statement = Statement {
            assign: Some(tmpvar.clone()),
            expression: Expr::Compare(CompareExpr::ICmp(comp, value1, value2)),
        };
        self.stmts.push(statement);
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch::if_else(tmpvar, if_addr, else_addr)),
            None,
        )))
    }

    fn if_zcmp(self, offset: i16, comp: IComparator) -> Fallible<Option<TranslateNext>> {
        let var = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, i32::from(offset));
        let else_addr = BlockId::from_addr(self.range.end);
        let tmpvar = self.var_id_gen.gen(Type::Boolean);
        let statement = Statement {
            assign: Some(tmpvar.clone()),
            expression: Expr::Compare(CompareExpr::ICmp(comp, var, Op::Const(Const::Int(0)))),
        };
        self.stmts.push(statement);
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch::if_else(tmpvar, if_addr, else_addr)),
            None,
        )))
    }

    fn if_acmp(self, offset: i16, comp: AComparator) -> Fallible<Option<TranslateNext>> {
        let value2 = self.state.pop();
        let value1 = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, i32::from(offset));
        let else_addr = BlockId::from_addr(self.range.end);
        let tmpvar = self.var_id_gen.gen(Type::Boolean);
        let statement = Statement {
            assign: Some(tmpvar.clone()),
            expression: Expr::Compare(CompareExpr::ACmp(comp, value1, value2)),
        };
        self.stmts.push(statement);
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch::if_else(tmpvar, if_addr, else_addr)),
            None,
        )))
    }

    fn if_acmpnull(self, offset: i16, comp: AComparator) -> Fallible<Option<TranslateNext>> {
        let value = self.state.pop();
        let if_addr = BlockId::from_addr_with_offset(self.range.start, i32::from(offset));
        let else_addr = BlockId::from_addr(self.range.end);
        let tmpvar = self.var_id_gen.gen(Type::Boolean);
        let statement = Statement {
            assign: Some(tmpvar.clone()),
            expression: Expr::Compare(CompareExpr::ACmp(comp, value, Op::Const(Const::Null))),
        };
        self.stmts.push(statement);
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch::if_else(tmpvar, if_addr, else_addr)),
            None,
        )))
    }

    fn object_new(&mut self, idx: u16) {
        let class = self.consts.get_class(ConstantIndex::from_u16(idx)).unwrap();
        let class_name = self.consts.get_utf8(class.name_index).unwrap();
        let var = self.var_id_gen.gen(Type::Reference);
        self.state.push(Op::Var(var.clone()));
        let statement = Statement {
            assign: Some(var),
            expression: Expr::New(class_name.clone()),
        };
        self.stmts.push(statement);
    }

    fn convert(&mut self, operation: ConvertOperation) {
        let value = self.state.pop();
        let target_type = match operation {
            ConvertOperation::IntToChar => Type::Int,
            ConvertOperation::IntToByte => Type::Int,
            ConvertOperation::IntToShort => Type::Int,
        };
        let result = self.var_id_gen.gen(target_type);
        self.state.push(Op::Var(result.clone()));
        let convert_expr = ConvertExpr {
            operation,
            operand: value,
        };
        let statement = Statement {
            assign: Some(result),
            expression: Expr::Convert(convert_expr),
        };
        self.stmts.push(statement);
    }

    fn monitor(&mut self, transition: MonitorStateTransition) {
        let objectref = self.state.pop();
        let statement = Statement {
            assign: None,
            expression: Expr::Monitor(objectref, transition),
        };
        self.stmts.push(statement);
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
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch {
                value,
                default,
                cases,
            }),
            None,
        )))
    }

    fn lookup_switch(self, lookup: &LookupSwitch) -> Fallible<Option<TranslateNext>> {
        let value = self.state.pop();
        let default = BlockId::from_addr_with_offset(self.range.start, lookup.default);
        let mut cases = Vec::with_capacity(lookup.pairs.len());
        for (compare_value, offset) in lookup.pairs.iter() {
            let addr = BlockId::from_addr_with_offset(self.range.start, *offset);
            cases.push((*compare_value, addr));
        }
        Ok(Some(TranslateNext(
            BranchStub::Switch(Switch {
                value,
                default,
                cases,
            }),
            None,
        )))
    }
}

fn translate_instructions(
    instrs: &mut Iterator<Item = &InstructionWithRange>,
    state: &mut StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
    stmts: &mut Vec<Statement>,
) -> Fallible<Option<TranslateNext>> {
    for InstructionWithRange { range, instr } in instrs {
        log::trace!("translating instruction {:?}", instr);
        let mut t = TranslateInstr {
            range,
            state,
            consts,
            var_id_gen,
            stmts,
        };
        match instr {
            // stack manipulation operations
            Instr::ALoad0 => t.load(0),
            Instr::ALoad1 => t.load(1),
            Instr::ALoad2 => t.load(2),
            Instr::ALoad3 => t.load(3),
            Instr::ALoad(idx) => t.load(*idx as usize),
            Instr::AStore0 => t.store(0),
            Instr::AStore1 => t.store(1),
            Instr::AStore2 => t.store(2),
            Instr::AStore3 => t.store(3),
            Instr::AStore(idx) => t.store(*idx as usize),
            Instr::ILoad(idx) => t.load(*idx as usize),
            Instr::IStore(idx) => t.store(*idx as usize),
            Instr::LLoad(idx) => t.load(*idx as usize),
            Instr::LStore(idx) => t.store(*idx as usize),
            Instr::FLoad(idx) => t.load(*idx as usize),
            Instr::FStore(idx) => t.store(*idx as usize),
            Instr::DLoad(idx) => t.load(*idx as usize),
            Instr::DStore(idx) => t.store(*idx as usize),
            Instr::Dup => t.duplicate(),
            Instr::Dup2 => t.duplicate2(),
            Instr::Pop => t.pop(1),
            Instr::Pop2 => t.pop(2),
            // arithmetic operations
            Instr::LCmp => t.lcmp(),
            Instr::LAdd => t.binary(Type::Long, BinaryOperation::Add),
            Instr::IAdd => t.binary(Type::Int, BinaryOperation::Add),
            Instr::ISub => t.binary(Type::Int, BinaryOperation::Sub),
            Instr::IAnd => t.binary(Type::Int, BinaryOperation::BitwiseAnd),
            Instr::IOr => t.binary(Type::Int, BinaryOperation::BitwiseOr),
            Instr::IXor => t.binary(Type::Int, BinaryOperation::BitwiseXor),
            Instr::IShL => t.binary(Type::Int, BinaryOperation::ShiftLeft),
            Instr::IShR => t.binary(Type::Int, BinaryOperation::ShiftRightArithmetic),
            Instr::IUShR => t.binary(Type::Int, BinaryOperation::ShiftRightLogical),
            Instr::IInc(idx, int) => t.iinc(*idx, i32::from(*int)),
            Instr::FCmpG => t.fcmp(NaNCmpMode::Greater),
            Instr::FCmpL => t.fcmp(NaNCmpMode::Less),
            Instr::DCmpG => t.dcmp(NaNCmpMode::Greater),
            Instr::DCmpL => t.dcmp(NaNCmpMode::Less),
            // conversion operations
            Instr::I2C => t.convert(ConvertOperation::IntToChar),
            Instr::I2B => t.convert(ConvertOperation::IntToByte),
            Instr::I2S => t.convert(ConvertOperation::IntToShort),
            // object operations
            Instr::New(idx) => t.object_new(*idx),
            Instr::MonitorEnter => t.monitor(MonitorStateTransition::Enter),
            Instr::MonitorExit => t.monitor(MonitorStateTransition::Exit),
            // field operations
            Instr::GetStatic(idx) => t.get_static(*idx),
            Instr::GetField(idx) => t.get_field(*idx),
            Instr::PutField(idx) => t.put_field(*idx),
            // array operations
            Instr::ANewArray(_) => t.array_new(Type::Reference),
            Instr::NewArray(atype) => t.array_new(Type::from_array_type(atype)),
            Instr::ArrayLength => t.array_length(),
            Instr::AaLoad => t.array_load(Type::Reference),
            Instr::BaLoad => t.array_load(Type::Byte),
            Instr::CaLoad => t.array_load(Type::Char),
            Instr::DaLoad => t.array_load(Type::Double),
            Instr::FaLoad => t.array_load(Type::Float),
            Instr::IaLoad => t.array_load(Type::Int),
            Instr::LaLoad => t.array_load(Type::Long),
            Instr::SaLoad => t.array_load(Type::Short),
            Instr::AaStore => t.array_store(Type::Reference),
            Instr::BaStore => t.array_store(Type::Byte),
            Instr::CaStore => t.array_store(Type::Char),
            Instr::DaStore => t.array_store(Type::Double),
            Instr::FaStore => t.array_store(Type::Float),
            Instr::IaStore => t.array_store(Type::Int),
            Instr::LaStore => t.array_store(Type::Long),
            Instr::SaStore => t.array_store(Type::Short),
            // contant load operations
            Instr::LdC(idx) => t.load_const(u16::from(*idx)),
            Instr::LdCW(idx) => t.load_const(*idx),
            Instr::LdC2W(idx) => t.load_const(*idx),
            Instr::IConst0 => t.push_const(Const::Int(0)),
            Instr::IConst1 => t.push_const(Const::Int(1)),
            Instr::IConst2 => t.push_const(Const::Int(2)),
            Instr::IConst3 => t.push_const(Const::Int(3)),
            Instr::IConst4 => t.push_const(Const::Int(4)),
            Instr::LConst0 => t.push_const(Const::Long(0)),
            Instr::LConst1 => t.push_const(Const::Long(1)),
            Instr::FConst0 => t.push_const(Const::Float(0.0)),
            Instr::FConst1 => t.push_const(Const::Float(1.0)),
            Instr::FConst2 => t.push_const(Const::Float(2.0)),
            Instr::DConst0 => t.push_const(Const::Double(0.0)),
            Instr::DConst1 => t.push_const(Const::Double(1.0)),
            Instr::AConstNull => t.push_const(Const::Null),
            Instr::BiPush(b) => t.push_const(Const::Int(i32::from(*b))),
            Instr::SiPush(s) => t.push_const(Const::Int(i32::from(*s))),
            // invoke operations
            Instr::InvokeSpecial(idx) => t.invoke(InvokeType::Special, *idx),
            Instr::InvokeStatic(idx) => t.invoke(InvokeType::Static, *idx),
            Instr::InvokeVirtual(idx) => t.invoke(InvokeType::Virtual, *idx),
            Instr::InvokeInterface(idx, _, _) => t.invoke(InvokeType::Interface, *idx),
            // branch operations
            Instr::Goto(offset) => return t.goto(*offset),
            Instr::Return => return t.ret(false),
            Instr::IReturn => return t.ret(true),
            Instr::AReturn => return t.ret(true),
            Instr::AThrow => return t.athrow(),
            Instr::IfLt(offset) => return t.if_zcmp(*offset, IComparator::Lt),
            Instr::IfLe(offset) => return t.if_zcmp(*offset, IComparator::Le),
            Instr::IfEq(offset) => return t.if_zcmp(*offset, IComparator::Eq),
            Instr::IfNe(offset) => return t.if_zcmp(*offset, IComparator::Ne),
            Instr::IfGe(offset) => return t.if_zcmp(*offset, IComparator::Ge),
            Instr::IfGt(offset) => return t.if_zcmp(*offset, IComparator::Gt),
            Instr::IfICmpLt(offset) => return t.if_icmp(*offset, IComparator::Lt),
            Instr::IfICmpLe(offset) => return t.if_icmp(*offset, IComparator::Le),
            Instr::IfICmpEq(offset) => return t.if_icmp(*offset, IComparator::Eq),
            Instr::IfICmpNe(offset) => return t.if_icmp(*offset, IComparator::Ne),
            Instr::IfICmpGe(offset) => return t.if_icmp(*offset, IComparator::Ge),
            Instr::IfICmpGt(offset) => return t.if_icmp(*offset, IComparator::Gt),
            Instr::IfACmpEq(offset) => return t.if_acmp(*offset, AComparator::Eq),
            Instr::IfACmpNe(offset) => return t.if_acmp(*offset, AComparator::Ne),
            Instr::IfNull(offset) => return t.if_acmpnull(*offset, AComparator::Eq),
            Instr::IfNonNull(offset) => return t.if_acmpnull(*offset, AComparator::Ne),
            Instr::TableSwitch(table) => return t.table_switch(table),
            Instr::LookupSwitch(lookup) => return t.lookup_switch(lookup),
            // misc operations
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
    match translate_instructions(
        &mut instrs,
        &mut state,
        &consts,
        var_id_gen,
        &mut statements,
    )? {
        Some(TranslateNext(branch_stub, exceptions)) => Ok(BasicBlock {
            address,
            incoming,
            statements,
            branch_stub,
            exceptions,
            outgoing: state,
        }),
        None => {
            let branch_stub = BranchStub::Switch(Switch::goto(BlockId(instr_block.range.end)));
            Ok(BasicBlock {
                address,
                incoming,
                statements,
                branch_stub,
                exceptions: Some(ExceptionHandlers),
                outgoing: state,
            })
        }
    }
}

pub fn translate_method(
    dasm: Disassembler,
    incoming: StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<BlockGraph> {
    let instr_block_map = InstructionBlockMap::build(dasm)?;
    let mut blocks = BlockGraph::new(incoming.clone());
    let mut remaining = vec![(BlockId::start(), incoming.new_with_same_shape(var_id_gen))];
    while let Some((addr, state)) = remaining.pop() {
        if !blocks.contains(addr) {
            let instr_block = instr_block_map.block_starting_at(addr.0);
            let block = translate_block(instr_block, state, &consts, var_id_gen)?;
            match block.branch_stub {
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
