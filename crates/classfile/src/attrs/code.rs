use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::super::instructions::Disassembler;
use super::super::{ConstantIndex, ConstantPool};
use super::{private, Attribute, Attributes, RawAttribute};
use ByteBuf;

#[derive(Debug)]
pub struct Code {
    pub max_stack: u16,
    pub max_locals: u16,
    code: ByteBuf,
    pub exception_table_len: u16,
    exception_table: ByteBuf,
    pub attributes: Attributes,
}

impl Code {
    pub fn bytecode(&self) -> &[u8] {
        self.code.as_ref()
    }

    pub fn disassemble(&self) -> Disassembler {
        Disassembler::new(self.code.clone())
    }

    pub fn exception_handlers(&self) -> ExceptionHandlers {
        ExceptionHandlers {
            len: self.exception_table_len,
            bytes: self.exception_table.clone(),
        }
    }
}

impl private::Sealed for Code {}

impl Attribute for Code {
    const NAME: &'static str = "Code";

    fn decode(raw: RawAttribute, consts: &ConstantPool) -> Fallible<Self> {
        let mut bytes = raw.bytes;
        let max_stack = bytes.read_u16::<BigEndian>()?;
        let max_locals = bytes.read_u16::<BigEndian>()?;
        let code_len = bytes.read_u32::<BigEndian>()?;
        let code = bytes.split_to(code_len as usize);
        let exception_table_len = bytes.read_u16::<BigEndian>()?;
        let exception_table_len_in_bytes =
            exception_table_len as usize * ::std::mem::size_of::<[u16; 4]>();
        let exception_table = bytes.split_to(exception_table_len_in_bytes);
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
pub struct ExceptionHandlers {
    len: u16,
    bytes: ByteBuf,
}

impl Iterator for ExceptionHandlers {
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

fn parse_exception_handler(bytes: &mut ByteBuf) -> Fallible<ExceptionHandler> {
    let start_pc = bytes.read_u16::<BigEndian>()?;
    let end_pc = bytes.read_u16::<BigEndian>()?;
    let handler_pc = bytes.read_u16::<BigEndian>()?;
    let catch_type = ConstantIndex::parse(bytes)?;
    Ok(ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc,
        catch_type,
    })
}
