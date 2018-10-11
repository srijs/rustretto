use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::super::instructions::Disassembler;
use super::super::{ConstantIndex, ConstantPool};
use super::{private, Attribute, Attributes, RawAttribute};

#[derive(Debug)]
pub struct Code<'a> {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: &'a [u8],
    pub exception_table_len: u16,
    pub exception_table: &'a [u8],
    pub attributes: Attributes,
}

impl<'a> Code<'a> {
    pub fn disassemble(&self) -> Disassembler<'a> {
        Disassembler::new(&self.code)
    }

    pub fn exception_handlers(&self) -> ExceptionHandlers<'a> {
        ExceptionHandlers {
            len: self.exception_table_len,
            bytes: self.exception_table,
        }
    }
}

impl<'a> private::Sealed for Code<'a> {}

impl<'a> Attribute<'a> for Code<'a> {
    const NAME: &'static str = "Code";

    fn decode(raw: RawAttribute<'a>, consts: &ConstantPool) -> Fallible<Self> {
        let mut bytes = raw.bytes.as_ref();
        let max_stack = bytes.read_u16::<BigEndian>()?;
        let max_locals = bytes.read_u16::<BigEndian>()?;
        let code_len = bytes.read_u32::<BigEndian>()?;
        let (code, mut bytes) = bytes.split_at(code_len as usize);
        let exception_table_len = bytes.read_u16::<BigEndian>()?;
        let exception_table_len_in_bytes =
            exception_table_len as usize * ::std::mem::size_of::<[u16; 4]>();
        let (exception_table, mut bytes) = bytes.split_at(exception_table_len_in_bytes);
        let attributes = Attributes::parse(&mut bytes, consts)?;
        Ok(Code {
            max_stack,
            max_locals,
            code,
            exception_table_len,
            exception_table,
            attributes,
        })
    }
}

#[derive(Debug)]
pub struct ExceptionHandlers<'a> {
    len: u16,
    bytes: &'a [u8],
}

impl<'a> Iterator for ExceptionHandlers<'a> {
    type Item = Fallible<ExceptionHandler>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(parse_exception_handler(&mut self.bytes))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.len as usize))
    }
}

#[derive(Debug)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: ConstantIndex,
}

fn parse_exception_handler(mut bytes: &[u8]) -> Fallible<ExceptionHandler> {
    let start_pc = bytes.read_u16::<BigEndian>()?;
    let end_pc = bytes.read_u16::<BigEndian>()?;
    let handler_pc = bytes.read_u16::<BigEndian>()?;
    let catch_type = ConstantIndex::parse(&mut bytes)?;
    Ok(ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc,
        catch_type,
    })
}
