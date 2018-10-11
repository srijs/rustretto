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

    pub fn get_stack_map_table(&self) -> Option<&StackMapTable> {
        if let Some(Attribute::StackMapTable(table)) = self.get("StackMapTable") {
            Some(table)
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
            Some("StackMapTable") => Attribute::StackMapTable(Self::parse_stack_map_table(&info)?),
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

    fn parse_verification_type_info(mut bytes: &[u8]) -> Fallible<VerificationTypeInfo> {
        match bytes.read_u8()? {
            0 => Ok(VerificationTypeInfo::Top),
            1 => Ok(VerificationTypeInfo::Integer),
            2 => Ok(VerificationTypeInfo::Float),
            3 => Ok(VerificationTypeInfo::Double),
            4 => Ok(VerificationTypeInfo::Long),
            5 => Ok(VerificationTypeInfo::Null),
            6 => Ok(VerificationTypeInfo::UninitializedThis),
            7 => Ok(VerificationTypeInfo::Object(ConstantIndex::parse(bytes)?)),
            8 => Ok(VerificationTypeInfo::Uninitialized(
                bytes.read_u16::<BigEndian>()?,
            )),
            x => bail!("unknown verification type tag {}", x),
        }
    }

    fn parse_stack_map_table(mut bytes: &[u8]) -> Fallible<StackMapTable> {
        let count = bytes.read_u16::<BigEndian>()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let frame_type = bytes.read_u8()?;
            if frame_type <= 63 {
                entries.push(StackMapTableEntry::SameFrame {
                    offset_delta: frame_type,
                });
            } else if frame_type >= 64 && frame_type <= 127 {
                let stack_item = Self::parse_verification_type_info(bytes)?;
                entries.push(StackMapTableEntry::SameLocals1StackItem {
                    offset_delta: frame_type - 64,
                    stack_item,
                });
            } else if frame_type == 247 {
                let offset_delta = bytes.read_u16::<BigEndian>()?;
                let stack_item = Self::parse_verification_type_info(bytes)?;
                entries.push(StackMapTableEntry::SameLocals1StackItemExtended {
                    offset_delta,
                    stack_item,
                });
            } else if frame_type >= 248 && frame_type <= 250 {
                let offset_delta = bytes.read_u16::<BigEndian>()?;
                entries.push(StackMapTableEntry::ChopFrame {
                    offset_delta,
                    k: 251 - frame_type,
                });
            } else if frame_type == 251 {
                let offset_delta = bytes.read_u16::<BigEndian>()?;
                entries.push(StackMapTableEntry::SameFrameExtended { offset_delta });
            } else if frame_type >= 252 && frame_type <= 254 {
                let offset_delta = bytes.read_u16::<BigEndian>()?;
                let k = frame_type - 251;
                let mut locals = Vec::with_capacity(k as usize);
                for _ in 0..k {
                    locals.push(Self::parse_verification_type_info(&mut bytes)?);
                }
                entries.push(StackMapTableEntry::AppendFrame {
                    offset_delta,
                    locals,
                });
            } else if frame_type == 255 {
                let offset_delta = bytes.read_u16::<BigEndian>()?;
                let number_of_locals = bytes.read_u16::<BigEndian>()?;
                let mut locals = Vec::with_capacity(number_of_locals as usize);
                for _ in 0..number_of_locals {
                    locals.push(Self::parse_verification_type_info(&mut bytes)?);
                }
                let number_of_stack_items = bytes.read_u16::<BigEndian>()?;
                let mut stack_items = Vec::with_capacity(number_of_stack_items as usize);
                for _ in 0..number_of_stack_items {
                    stack_items.push(Self::parse_verification_type_info(&mut bytes)?);
                }
                entries.push(StackMapTableEntry::FullFrame {
                    offset_delta,
                    locals,
                    stack_items,
                });
            } else {
                bail!("unknown frame type {}", frame_type)
            }
        }
        Ok(StackMapTable { entries })
    }
}

#[derive(Debug)]
pub enum Attribute {
    ConstantValue(ConstantValue),
    Code(Code),
    SourceFile(SourceFile),
    LineNumberTable(LineNumberTable),
    StackMapTable(StackMapTable),
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

#[derive(Debug)]
pub struct StackMapTable {
    pub entries: Vec<StackMapTableEntry>,
}

#[derive(Debug)]
pub enum StackMapTableEntry {
    SameFrame {
        offset_delta: u8,
    },
    SameLocals1StackItem {
        offset_delta: u8,
        stack_item: VerificationTypeInfo,
    },
    SameLocals1StackItemExtended {
        offset_delta: u16,
        stack_item: VerificationTypeInfo,
    },
    ChopFrame {
        k: u8,
        offset_delta: u16,
    },
    SameFrameExtended {
        offset_delta: u16,
    },
    AppendFrame {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
    },
    FullFrame {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
        stack_items: Vec<VerificationTypeInfo>,
    },
}

#[derive(Debug)]
pub enum VerificationTypeInfo {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    UninitializedThis,
    Object(ConstantIndex),
    Uninitialized(u16),
}
