extern crate classfile;
#[macro_use]
extern crate failure;
extern crate petgraph;

use std::fmt;
use std::fs;

use classfile::attrs::stack_map_table::VerificationTypeInfo;
use classfile::attrs::Code;
use classfile::constant_pool::Constant;
use classfile::descriptors::{
    ArrayType, BaseType, FieldType, ParameterDescriptor, ReturnTypeDescriptor,
};
use classfile::instructions::{Disassembler, Instr};
use classfile::{ClassFile, ConstantIndex, ConstantPool};
use failure::Fallible;

mod disasm;
mod frame;
mod generate;
mod graph;
mod utils;

use disasm::{InstructionBlock, InstructionBlockMap, InstructionWithRange};
use frame::StackAndLocals;
use graph::BlockGraph;
use utils::MinHeap;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Type {
    info: VerificationTypeInfo,
}

impl Type {
    pub fn int() -> Self {
        Type {
            info: VerificationTypeInfo::Integer,
        }
    }

    pub fn string() -> Self {
        Type {
            info: VerificationTypeInfo::Object("java.lang.String".to_owned()),
        }
    }

    pub fn from_field_type(field_type: FieldType) -> Self {
        match field_type {
            FieldType::Base(base_type) => match base_type {
                classfile::descriptors::BaseType::Boolean => Self::int(),
                _ => unimplemented!("unsupported base type {:?}", base_type),
            },
            FieldType::Object(object_type) => Type {
                info: VerificationTypeInfo::Object(object_type.class_name),
            },
            FieldType::Array(array_type) => {
                let class_name = array_type_to_class_name(&array_type);
                Type {
                    info: VerificationTypeInfo::Object(class_name),
                }
            }
            _ => unimplemented!("unsupported field type {:?}", field_type),
        }
    }
}

pub fn array_type_to_class_name(array_type: &ArrayType) -> String {
    let mut output = "[".to_owned();
    let mut field_type = &*array_type.component_type;
    loop {
        match field_type {
            FieldType::Base(base_type) => {
                match base_type {
                    BaseType::Byte => output.push('B'),
                    BaseType::Char => output.push('C'),
                    BaseType::Double => output.push('D'),
                    BaseType::Float => output.push('F'),
                    BaseType::Int => output.push('I'),
                    BaseType::Long => output.push('J'),
                    BaseType::Short => output.push('S'),
                    BaseType::Boolean => output.push('Z'),
                };
                return output;
            }
            FieldType::Object(object_type) => {
                output.push('L');
                output.push_str(&object_type.class_name);
                output.push(';');
                return output;
            }
            FieldType::Array(array_type) => {
                output.push('[');
                field_type = &*array_type.component_type;
            }
        }
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.info {
            VerificationTypeInfo::Integer => write!(f, "int"),
            VerificationTypeInfo::Object(ref name) => write!(f, "{}", name),
            _ => self.info.fmt(f),
        }
    }
}

#[derive(Debug)]
struct BlockId(usize);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VarId(pub Type, pub u64);

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
struct InvokeExpr {
    pub target: InvokeTarget,
    pub index: ConstantIndex,
    pub args: Vec<VarId>,
}

#[derive(Debug)]
enum Expr {
    Var(VarId),
    Const(ConstantIndex),
    ConstInt(i32),
    ConstString(String),
    GetStatic(ConstantIndex),
    Invoke(InvokeExpr),
}

#[derive(Debug)]
struct ExceptionHandlers; // TODO

#[derive(Debug)]
enum BranchStub {
    Goto(u32),
    IfEq(VarId, u32, u32),
    Return(Option<VarId>),
    Invoke(Option<VarId>, InvokeExpr, u32),
}

#[derive(Debug)]
enum Branch {
    Return(Option<VarId>),
    IfEq(VarId, BlockId, BlockId),
}

#[derive(Debug)]
struct Statement {
    assign: Option<VarId>,
    expression: Expr,
}

#[derive(Debug)]
struct BasicBlock {
    address: u32,
    incoming: StackAndLocals,
    statements: Vec<Statement>,
    branch_stub: BranchStub,
    exceptions: Option<ExceptionHandlers>,
    outgoing: StackAndLocals,
}

enum TranslateNext {
    Statement(Statement),
    Branch(BranchStub, Option<ExceptionHandlers>),
}

fn translate_invoke(
    invoke: InvokeType,
    index: ConstantIndex,
    state: &mut StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<(Option<VarId>, InvokeExpr)> {
    let method = consts.get_method_ref(index).unwrap();
    let method_args_len = method.descriptor.params.len();
    let args = state.pop_n(method_args_len);
    let expr = match invoke {
        InvokeType::Static => InvokeExpr {
            target: InvokeTarget::Static,
            index,
            args,
        },
        InvokeType::Special => InvokeExpr {
            target: InvokeTarget::Special(state.pop()),
            index,
            args,
        },
        InvokeType::Virtual => InvokeExpr {
            target: InvokeTarget::Virtual(state.pop()),
            index,
            args,
        },
    };
    let return_type = match method.descriptor.ret {
        ReturnTypeDescriptor::Void => None,
        ReturnTypeDescriptor::Field(field_type) => Some(Type::from_field_type(field_type)),
    };
    let return_var = return_type.map(|t| var_id_gen.gen(t));
    if let Some(ref var) = return_var {
        state.push(var.clone());
    }
    Ok((return_var, expr))
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
                let (bind, expr) = translate_invoke(
                    InvokeType::Special,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                let statement = Statement {
                    assign: bind,
                    expression: Expr::Invoke(expr),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::InvokeStatic(idx) => {
                let (bind, expr) = translate_invoke(
                    InvokeType::Static,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                let statement = Statement {
                    assign: bind,
                    expression: Expr::Invoke(expr),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::InvokeVirtual(idx) => {
                let (bind, expr) = translate_invoke(
                    InvokeType::Virtual,
                    ConstantIndex::from_u16(*idx),
                    state,
                    consts,
                    var_id_gen,
                )?;
                let statement = Statement {
                    assign: bind,
                    expression: Expr::Invoke(expr),
                };
                return Ok(Some(TranslateNext::Statement(statement)));
            }
            Instr::Return => {
                return Ok(Some(TranslateNext::Branch(
                    BranchStub::Return(None),
                    Some(ExceptionHandlers),
                )));
            }
            Instr::IfEq(offset) => {
                let var = state.pop();
                let if_addr = (range.start as i64 + *offset as i64) as u32;
                let else_addr = range.end;
                return Ok(Some(TranslateNext::Branch(
                    BranchStub::IfEq(var, if_addr, else_addr),
                    None,
                )));
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

fn translate_method(
    dasm: Disassembler,
    incoming: StackAndLocals,
    consts: &ConstantPool,
    var_id_gen: &mut VarIdGen,
) -> Fallible<BlockGraph> {
    let instr_block_map = InstructionBlockMap::build(dasm)?;
    let mut blocks = BlockGraph::new();
    let mut remaining = MinHeap::singleton(0u32, incoming);
    while let Some((addr, state)) = remaining.pop() {
        if !blocks.contains(addr) {
            let instr_block = instr_block_map.block_starting_at(addr);
            let block = translate_block(instr_block, state, &consts, var_id_gen)?;
            match block.branch_stub {
                BranchStub::Goto(addr) => {
                    remaining.push(addr, block.outgoing.new_with_same_shape(var_id_gen));
                }
                BranchStub::IfEq(_, if_addr, else_addr) => {
                    remaining.push(if_addr, block.outgoing.new_with_same_shape(var_id_gen));
                    remaining.push(else_addr, block.outgoing.new_with_same_shape(var_id_gen));
                }
                BranchStub::Invoke(_, _, addr) => {
                    remaining.push(addr, block.outgoing.new_with_same_shape(var_id_gen));
                }
                BranchStub::Return(_) => {}
            }
            blocks.insert(block);
        }
    }
    blocks.calculate_edges();
    Ok(blocks)
}

fn main() {
    let file = fs::File::open("test-jar/Basic.class").unwrap();
    let cf = ClassFile::parse(file).unwrap();

    let class = cf.get_this_class();
    let class_name = cf.constant_pool.get_utf8(class.name_index).unwrap();

    generate::gen_prelude(&cf);
    for method in cf.methods.iter() {
        let mut var_id_gen = VarIdGen { next_id: 0 };
        let name = cf.constant_pool.get_utf8(method.name_index).unwrap();
        let mut args = Vec::new();
        if name == "<init>" {
            let arg_type = Type {
                info: VerificationTypeInfo::UninitializedThis,
            };
            args.push(var_id_gen.gen(arg_type));
        } else {
            let arg_type = Type {
                info: VerificationTypeInfo::Object(class_name.to_owned()),
            };
            args.push(var_id_gen.gen(arg_type));
        }
        for ParameterDescriptor::Field(field_type) in method.descriptor.params.iter() {
            args.push(var_id_gen.gen(Type::from_field_type(field_type.clone())));
        }
        let code = method.attributes.get::<Code>().unwrap();
        let state = StackAndLocals::new(code.max_stack, code.max_locals, &args);
        let blocks = translate_method(
            code.disassemble(),
            state,
            &cf.constant_pool,
            &mut var_id_gen,
        ).unwrap();
        generate::gen_method(&cf, &method, &blocks, &cf.constant_pool, &mut var_id_gen);
    }
}
