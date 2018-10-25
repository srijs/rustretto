extern crate classfile;
#[macro_use]
extern crate failure;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::iter::FromIterator;
use std::sync::Arc;

use classfile::attrs::stack_map_table::VerificationTypeInfo;
use classfile::attrs::Code;
use classfile::constant_pool::Constant;
use classfile::descriptors::{FieldType, ReturnTypeDescriptor};
use classfile::instructions::{Disassembler, Instr};
use classfile::{ClassFile, ConstantIndex, ConstantPool};
use failure::Fallible;

mod disasm;
mod dump;
mod utils;

use disasm::{InstructionBlock, InstructionBlockMap, InstructionWithRange};
use utils::MinHeap;

#[derive(Clone)]
struct Type {
    info: Arc<VerificationTypeInfo>,
}

impl Type {
    pub fn int() -> Self {
        Type {
            info: Arc::new(VerificationTypeInfo::Integer),
        }
    }

    pub fn string() -> Self {
        Type {
            info: Arc::new(VerificationTypeInfo::Object("java.lang.String".to_owned())),
        }
    }

    pub fn from_field_type(field_type: FieldType) -> Self {
        match field_type {
            FieldType::Base(base_type) => match base_type {
                classfile::descriptors::BaseType::Boolean => Self::int(),
                _ => unimplemented!("unsupported base type {:?}", base_type),
            },
            FieldType::Object(object_type) => Type {
                info: Arc::new(VerificationTypeInfo::Object(object_type.class_name)),
            },
            _ => unimplemented!("unsupported field type {:?}", field_type),
        }
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.info {
            VerificationTypeInfo::Integer => write!(f, "int"),
            VerificationTypeInfo::Object(ref name) => write!(f, "{}", name),
            _ => self.info.fmt(f),
        }
    }
}

#[derive(Debug)]
struct BlockId(usize);

#[derive(Clone)]
struct VarId(Type, u64);

impl fmt::Debug for VarId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "v{}", self.1)
    }
}

struct VarIdGen {
    next_id: u64,
}

impl VarIdGen {
    fn gen(&mut self, var_type: Type) -> VarId {
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
enum InvokeTarget {
    Static,
    Special(VarId),
    Virtual(VarId),
}

#[derive(Debug)]
enum Expr {
    Var(VarId),
    Const(ConstantIndex),
    ConstInt(i32),
    ConstString(String),
    GetStatic(ConstantIndex),
    Invoke(InvokeTarget, ConstantIndex, Vec<VarId>),
}

#[derive(Debug)]
struct ExceptionHandlers; // TODO

#[derive(Debug)]
enum BranchStub {
    IfEq(VarId, u32, u32),
    Return(Option<VarId>),
    IfException(ExceptionHandlers, u32),
}

#[derive(Debug)]
enum Branch {
    Return(Option<VarId>),
    IfEq(VarId, BlockId, BlockId),
}

#[derive(Clone, Debug)]
struct StackAndLocals {
    stack: Vec<VarId>,
    locals: BTreeMap<usize, VarId>,
}

impl StackAndLocals {
    fn new(max_stack: u16, _max_locals: u16, args: &[VarId]) -> StackAndLocals {
        let stack = Vec::with_capacity(max_stack as usize);
        let mut locals = BTreeMap::new();
        locals.extend(args.into_iter().cloned().enumerate());
        StackAndLocals { stack, locals }
    }

    fn new_with_same_shape(&self, var_id_gen: &mut VarIdGen) -> Self {
        let stack = self
            .stack
            .iter()
            .map(|v| var_id_gen.gen(v.0.clone()))
            .collect();
        let locals = self
            .locals
            .iter()
            .map(|(i, v)| (*i, var_id_gen.gen(v.0.clone())))
            .collect();
        StackAndLocals { stack, locals }
    }

    fn pop(&mut self) -> VarId {
        self.stack.pop().unwrap()
    }

    fn pop_n(&mut self, n: usize) -> Vec<VarId> {
        let index = self.stack.len() - n;
        self.stack.split_off(index)
    }

    fn push(&mut self, var: VarId) {
        self.stack.push(var);
    }

    fn load(&mut self, idx: usize) {
        self.stack.push(self.locals[&idx].clone());
    }

    fn store(&mut self, idx: usize) {
        self.locals.insert(idx, self.stack.pop().unwrap());
    }
}

#[derive(Debug)]
struct Statement {
    assign: Option<VarId>,
    expression: Expr,
}

#[derive(Debug)]
struct BasicBlock {
    incoming: StackAndLocals,
    statements: Vec<Statement>,
    branch_stub: BranchStub,
    outgoing: StackAndLocals,
}

enum TranslateNext {
    Statement(Statement),
    Branch(BranchStub),
}

fn translate_invoke(
    invoke: InvokeType,
    index: ConstantIndex,
    state: &mut StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<Statement> {
    let method = consts.get_method_ref(index).unwrap();
    let method_args_len = method.descriptor.params.len();
    let invoke_args = state.pop_n(method_args_len);
    let expression = match invoke {
        InvokeType::Static => Expr::Invoke(InvokeTarget::Static, index, invoke_args),
        InvokeType::Special => Expr::Invoke(InvokeTarget::Special(state.pop()), index, invoke_args),
        InvokeType::Virtual => Expr::Invoke(InvokeTarget::Virtual(state.pop()), index, invoke_args),
    };
    let return_type = match method.descriptor.ret {
        ReturnTypeDescriptor::Void => None,
        ReturnTypeDescriptor::Field(field_type) => Some(Type::from_field_type(field_type)),
    };
    let return_var = return_type.map(|t| var_id_gen.gen(t));
    if let Some(ref var) = return_var {
        state.push(var.clone());
    }
    Ok(Statement {
        assign: return_var,
        expression,
    })
}

fn translate_next(
    instrs: &mut Iterator<Item = &InstructionWithRange>,
    state: &mut StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<Option<TranslateNext>> {
    for InstructionWithRange { range, instr } in instrs {
        match instr {
            Instr::ALoad0 => {
                state.load(0);
            }
            Instr::ALoad1 => {
                state.load(1);
            }
            Instr::AStore1 => {
                state.store(1);
            }
            Instr::IConst0 => {
                let var = var_id_gen.gen(Type::int());
                state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::ConstInt(0),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::GetStatic(idx) => {
                let field = consts.get_field_ref(ConstantIndex::from_u16(*idx)).unwrap();
                let var = var_id_gen.gen(Type::from_field_type(field.descriptor));
                state.push(var.clone());
                let statement = Statement {
                    assign: Some(var),
                    expression: Expr::GetStatic(ConstantIndex::from_u16(*idx)),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::LdC(idx) => match consts.get_info(ConstantIndex::from_u8(*idx)).unwrap() {
                Constant::String(ref string_const) => {
                    let string_value = consts.get_utf8(string_const.string_index).unwrap();
                    let var = var_id_gen.gen(Type::string());
                    state.push(var.clone());
                    let statement = Statement {
                        assign: Some(var),
                        expression: Expr::ConstString(string_value.to_owned()),
                    };
                    return Ok(Some(TranslateNext::Statement(statement)));
                }
                constant => bail!("unsupported load of constant {:?}", constant),
            },
            Instr::InvokeSpecial(idx) => {
                let statement = translate_invoke(
                    InvokeType::Special,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::InvokeStatic(idx) => {
                let statement = translate_invoke(
                    InvokeType::Static,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::InvokeVirtual(idx) => {
                let statement = translate_invoke(
                    InvokeType::Virtual,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::Return => {
                return Ok(Some(TranslateNext::Branch(BranchStub::Return(None))));
            }
            Instr::IfEq(offset) => {
                let var = state.pop();
                let if_addr = (range.start as i64 + *offset as i64) as u32;
                let else_addr = range.end;
                return Ok(Some(TranslateNext::Branch(BranchStub::IfEq(
                    var, if_addr, else_addr,
                ))));
            }
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
    let mut state = incoming.clone();
    let mut statements = Vec::new();
    let mut instrs = instr_block.instrs.iter();
    loop {
        match translate_next(&mut instrs, &mut state, &consts, var_id_gen)? {
            Some(TranslateNext::Statement(stmt)) => {
                statements.push(stmt);
            }
            Some(TranslateNext::Branch(branch_stub)) => {
                return Ok(BasicBlock {
                    incoming,
                    statements,
                    branch_stub,
                    outgoing: state,
                });
            }
            None => {
                let branch_stub = BranchStub::IfException(ExceptionHandlers, instr_block.range.end);
                return Ok(BasicBlock {
                    incoming,
                    statements,
                    branch_stub,
                    outgoing: state,
                });
            }
        }
    }
}

fn translate(
    dasm: Disassembler,
    incoming: StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<BTreeMap<u32, BasicBlock>> {
    let instr_block_map = InstructionBlockMap::build(dasm)?;
    let mut blocks = BTreeMap::new();
    let mut remaining = MinHeap::singleton(0u32, incoming);
    while let Some((addr, state)) = remaining.pop() {
        if !blocks.contains_key(&addr) {
            let instr_block = instr_block_map.block_starting_at(addr);
            let block = translate_block(instr_block, state, &consts, var_id_gen)?;
            match block.branch_stub {
                BranchStub::IfException(_, else_index) => {
                    remaining.push(else_index, block.outgoing.new_with_same_shape(var_id_gen));
                }
                BranchStub::IfEq(_, if_index, else_index) => {
                    remaining.push(if_index, block.outgoing.new_with_same_shape(var_id_gen));
                    remaining.push(else_index, block.outgoing.new_with_same_shape(var_id_gen));
                }
                BranchStub::Return(_) => {}
            }
            blocks.insert(addr, block);
        }
    }
    Ok(blocks)
}

fn main() {
    let file = fs::File::open("test-jar/Test.class").unwrap();
    let cf = ClassFile::parse(file).unwrap();

    for method in cf.methods {
        let mut var_id_gen = VarIdGen { next_id: 0 };
        let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
        let mut args = Vec::new();
        if name == "<init>" {
            let arg_type = Type {
                info: Arc::new(VerificationTypeInfo::UninitializedThis),
            };
            args.push(var_id_gen.gen(arg_type));
        }
        let code = method.attributes.get::<Code>().unwrap();
        let state = StackAndLocals::new(code.max_stack, code.max_locals, &args);
        let blocks = translate(
            code.disassemble(),
            state,
            &cf.constant_pool,
            &mut var_id_gen,
        ).unwrap();
        println!("⭆ Method {}", name);
        for (idx, block) in blocks.iter() {
            dump::dump_basic_block(*idx, &block, &cf.constant_pool);
        }
    }
}
