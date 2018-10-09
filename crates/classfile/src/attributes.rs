use std::collections::HashMap;
use std::io::Read;

use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::instructions::Disassembler;
use super::{ConstantIndex, ConstantPool};

#[derive(Debug)]
pub struct Attributes {
    attrs: HashMap<String, Attribute>,
    consts: ConstantPool,
}

impl Attributes {
    pub(crate) fn parse<R: Read>(mut reader: R, consts: &ConstantPool) -> Fallible<Self> {
        let count = reader.read_u16::<BigEndian>()?;
        let mut attrs = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let (name, attr) = AttributeParser::new(&mut reader).parse(consts)?;
            attrs.insert(name, attr);
        }
        Ok(Attributes {
            attrs,
            consts: consts.clone(),
        })
    }

    pub fn get_source_file(&self) -> Option<&str> {
        if let Some(Attribute::SourceFile(inner)) = self.get("SourceFile") {
            self.consts.get_utf8(inner.sourcefile_index)
        } else {
            None
        }
    }

    pub fn get_code(&self) -> Option<&Code> {
        if let Some(Attribute::Code(code)) = self.get("Code") {
            Some(code)
        } else {
            None
        }
    }

    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.attrs.get(name)
    }
}

struct AttributeParser<R> {
    reader: R,
}

impl<R: Read> AttributeParser<R> {
    fn new(reader: R) -> Self {
        AttributeParser { reader }
    }

    fn parse(&mut self, constants: &ConstantPool) -> Fallible<(String, Attribute)> {
        let name_index = ConstantIndex::parse(&mut self.reader)?;
        let name = constants.get_utf8(name_index).unwrap();
        let len = self.reader.read_u32::<BigEndian>()?;
        let mut info = vec![0u8; len as usize];
        self.reader.read_exact(&mut info)?;
        let attr = match constants.get_utf8(name_index) {
            Some("ConstantValue") => Attribute::ConstantValue(Self::parse_constant_value(&info)?),
            Some("Code") => Attribute::Code(Self::parse_code(&info, constants)?),
            Some("SourceFile") => Attribute::SourceFile(Self::parse_source_file(&info)?),
            Some("LineNumberTable") => {
                Attribute::LineNumberTable(Self::parse_line_number_table(&info)?)
            }
            _ => Attribute::Unknown(Unknown { info }),
        };
        Ok((name.to_owned(), attr))
    }

    fn parse_constant_value(bytes: &[u8]) -> Fallible<ConstantValue> {
        let value_index = ConstantIndex::parse(bytes)?;
        Ok(ConstantValue { value_index })
    }

    fn parse_code(mut bytes: &[u8], constants: &ConstantPool) -> Fallible<Code> {
        let max_stack = bytes.read_u16::<BigEndian>()?;
        let max_locals = bytes.read_u16::<BigEndian>()?;
        let code_len = bytes.read_u32::<BigEndian>()?;
        let mut code = vec![0u8; code_len as usize];
        bytes.read_exact(&mut code)?;
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
        let attributes = Attributes::parse(&mut bytes, constants)?;
        Ok(Code {
            max_stack,
            max_locals,
            code,
            exception_table,
            attributes,
        })
    }

    fn parse_source_file(bytes: &[u8]) -> Fallible<SourceFile> {
        let sourcefile_index = ConstantIndex::parse(bytes)?;
        Ok(SourceFile { sourcefile_index })
    }

    fn parse_line_number_table(mut bytes: &[u8]) -> Fallible<LineNumberTable> {
        let len = bytes.read_u16::<BigEndian>()?;
        let mut entries = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let start_pc = bytes.read_u16::<BigEndian>()?;
            let line_number = bytes.read_u16::<BigEndian>()?;
            entries.push(LineNumberTableEntry {
                start_pc,
                line_number,
            })
        }
        Ok(LineNumberTable { entries })
    }
}

#[derive(Debug)]
pub enum Attribute {
    ConstantValue(ConstantValue),
    Code(Code),
    SourceFile(SourceFile),
    LineNumberTable(LineNumberTable),
    Unknown(Unknown),
}

#[derive(Debug)]
pub struct Unknown {
    pub info: Vec<u8>,
}

#[derive(Debug)]
pub struct ConstantValue {
    pub value_index: ConstantIndex,
}

#[derive(Debug)]
pub struct Code {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionTableEntry>,
    pub attributes: Attributes,
}

impl Code {
    pub fn decode(&self) -> Disassembler {
        Disassembler::new(&self.code)
    }
}

#[derive(Debug)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: ConstantIndex,
}

#[derive(Debug)]
pub struct SourceFile {
    pub sourcefile_index: ConstantIndex,
}

#[derive(Debug)]
pub struct LineNumberTable {
    pub entries: Vec<LineNumberTableEntry>,
}

#[derive(Debug)]
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16,
}
