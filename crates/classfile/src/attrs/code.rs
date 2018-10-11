use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::super::instructions::Disassembler;
use super::super::{ConstantIndex, ConstantPool};
use super::{Attribute, Attributes};

#[derive(Debug)]
pub struct Code<'a> {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: &'a [u8],
    pub exception_table: Vec<ExceptionTableEntry>,
    pub attributes: Attributes,
}

impl<'a> Code<'a> {
    pub fn disassemble(&self) -> Disassembler<'a> {
        Disassembler::new(&self.code)
    }
}

impl<'a> Attribute<'a> for Code<'a> {
    const NAME: &'static str = "Code";

    fn decode(mut bytes: &'a [u8], consts: &ConstantPool) -> Fallible<Self> {
        let max_stack = bytes.read_u16::<BigEndian>()?;
        let max_locals = bytes.read_u16::<BigEndian>()?;
        let code_len = bytes.read_u32::<BigEndian>()?;
        let (code, mut bytes) = bytes.split_at(code_len as usize);
        let exception_table_len = bytes.read_u16::<BigEndian>()?;
        let mut exception_table = Vec::with_capacity(exception_table_len as usize);
        for _ in 0..exception_table_len {
            let start_pc = bytes.read_u16::<BigEndian>()?;
            let end_pc = bytes.read_u16::<BigEndian>()?;
            let handler_pc = bytes.read_u16::<BigEndian>()?;
            let catch_type = ConstantIndex::parse(&mut bytes)?;
            exception_table.push(ExceptionTableEntry {
                start_pc,
                end_pc,
                handler_pc,
                catch_type,
            });
        }
        let attributes = Attributes::parse(&mut bytes, consts)?;
        Ok(Code {
            max_stack,
            max_locals,
            code,
            exception_table,
            attributes,
        })
    }
}

#[derive(Debug)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: ConstantIndex,
}
