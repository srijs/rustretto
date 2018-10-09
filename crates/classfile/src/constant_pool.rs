use std::fmt;
use std::io::Read;
use std::ops::Index;
use std::sync::Arc;

use byteorder::{BigEndian, ReadBytesExt};
use cesu8::from_java_cesu8;
use failure::Fallible;

const CONSTANT_CLASS: u8 = 7;
const CONSTANT_FIELD_REF: u8 = 9;
const CONSTANT_METHOD_REF: u8 = 10;
const CONSTANT_IFACE_METHOD_REF: u8 = 11;
const CONSTANT_STRING: u8 = 8;
const CONSTANT_INTEGER: u8 = 3;
const CONSTANT_FLOAT: u8 = 4;
const CONSTANT_LONG: u8 = 5;
const CONSTANT_DOUBLE: u8 = 6;
const CONSTANT_NAME_AND_TYPE: u8 = 12;
const CONSTANT_UTF8: u8 = 1;
const CONSTANT_METHOD_HANDLE: u8 = 15;
const CONSTANT_METHOD_TYPE: u8 = 16;
const CONSTANT_INVOKE_DYNAMIC: u8 = 18;

#[derive(Clone, Debug)]
pub struct ConstantPool {
    vec: Arc<[Constant]>,
}

impl ConstantPool {
    pub(crate) fn parse<R: Read>(reader: R) -> Fallible<Self> {
        let mut parser = ConstantPoolParser::new(reader);

        let mut vec = Vec::new();
        parser.parse(&mut vec)?;

        Ok(ConstantPool { vec: vec.into() })
    }

    pub fn get_info(&self, idx: ConstantIndex) -> Option<&Constant> {
        if idx.0 > 0 {
            self.vec.get(idx.0 as usize - 1)
        } else {
            None
        }
    }

    pub fn get_utf8(&self, idx: ConstantIndex) -> Option<&str> {
        if let Some(&Constant::Utf8(Utf8Constant { ref string })) = self.get_info(idx) {
            Some(string)
        } else {
            None
        }
    }

    pub fn get_class(&self, idx: ConstantIndex) -> Option<&ClassConstant> {
        if let Some(&Constant::Class(ref inner)) = self.get_info(idx) {
            Some(inner)
        } else {
            None
        }
    }

    pub fn get_name_and_type(&self, idx: ConstantIndex) -> Option<&NameAndTypeConstant> {
        if let Some(&Constant::NameAndType(ref inner)) = self.get_info(idx) {
            Some(inner)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConstantIndex(pub(crate) u16);

impl ConstantIndex {
    pub(crate) fn parse<R: Read>(mut reader: R) -> Fallible<ConstantIndex> {
        Ok(ConstantIndex(reader.read_u16::<BigEndian>()?))
    }

    pub fn from_u8(idx: u8) -> Self {
        ConstantIndex(idx as u16)
    }

    pub fn from_u16(idx: u16) -> Self {
        ConstantIndex(idx)
    }

    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl Index<ConstantIndex> for ConstantPool {
    type Output = Constant;

    fn index(&self, idx: ConstantIndex) -> &Constant {
        &self.vec[idx.0 as usize - 1]
    }
}

struct ConstantPoolParser<R> {
    reader: R,
}

impl<R: Read> ConstantPoolParser<R> {
    pub fn new(reader: R) -> Self {
        ConstantPoolParser { reader }
    }

    fn parse(&mut self, vec: &mut Vec<Constant>) -> Fallible<()> {
        let count = self.reader.read_u16::<BigEndian>()?;
        vec.reserve(count as usize - 1);
        for _i in 1..count {
            let tag = self.reader.read_u8()?;
            let info = match tag {
                CONSTANT_CLASS => Constant::Class(self.parse_constant_class_info()?),
                CONSTANT_FIELD_REF => Constant::FieldRef(self.parse_constant_field_ref_info()?),
                CONSTANT_METHOD_REF => Constant::MethodRef(self.parse_constant_method_ref_info()?),
                CONSTANT_IFACE_METHOD_REF => {
                    Constant::InterfaceMethodRef(self.parse_constant_iface_method_ref_info()?)
                }
                CONSTANT_STRING => Constant::String(self.parse_constant_string_info()?),
                CONSTANT_INTEGER => Constant::Integer(self.parse_constant_integer_info()?),
                CONSTANT_FLOAT => Constant::Float(self.parse_constant_float_info()?),
                CONSTANT_LONG => Constant::Long(self.parse_constant_long_info()?),
                CONSTANT_DOUBLE => Constant::Double(self.parse_constant_double_info()?),
                CONSTANT_NAME_AND_TYPE => {
                    Constant::NameAndType(self.parse_constant_name_and_type_info()?)
                }
                CONSTANT_UTF8 => Constant::Utf8(self.parse_constant_utf8_info()?),
                CONSTANT_METHOD_HANDLE => {
                    Constant::MethodHandle(self.parse_constant_method_handle_info()?)
                }
                CONSTANT_METHOD_TYPE => {
                    Constant::MethodType(self.parse_constant_method_type_info()?)
                }
                CONSTANT_INVOKE_DYNAMIC => {
                    Constant::InvokeDynamic(self.parse_constant_invoke_dynamic_info()?)
                }
                _ => bail!("unknown constant tag {}", tag),
            };
            vec.push(info);

            // Long and Double constants take up 2 entries in the pool.
            if tag == CONSTANT_LONG || tag == CONSTANT_DOUBLE {
                vec.push(Constant::Unusable);
            }
        }
        Ok(())
    }

    fn parse_constant_class_info(&mut self) -> Fallible<ClassConstant> {
        let name_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(ClassConstant { name_index })
    }

    fn parse_constant_field_ref_info(&mut self) -> Fallible<FieldRefConstant> {
        let class_index = ConstantIndex::parse(&mut self.reader)?;
        let name_and_type_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(FieldRefConstant {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_constant_method_ref_info(&mut self) -> Fallible<MethodRefConstant> {
        let class_index = ConstantIndex::parse(&mut self.reader)?;
        let name_and_type_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(MethodRefConstant {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_constant_iface_method_ref_info(&mut self) -> Fallible<InterfaceMethodRefConstant> {
        let class_index = ConstantIndex::parse(&mut self.reader)?;
        let name_and_type_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(InterfaceMethodRefConstant {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_constant_string_info(&mut self) -> Fallible<StringConstant> {
        let string_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(StringConstant { string_index })
    }

    fn parse_constant_integer_info(&mut self) -> Fallible<IntegerConstant> {
        let value = self.reader.read_i32::<BigEndian>()?;
        Ok(IntegerConstant { value })
    }

    fn parse_constant_float_info(&mut self) -> Fallible<FloatConstant> {
        let value = self.reader.read_f32::<BigEndian>()?;
        Ok(FloatConstant { value })
    }

    fn parse_constant_long_info(&mut self) -> Fallible<LongConstant> {
        let value = self.reader.read_i64::<BigEndian>()?;
        Ok(LongConstant { value })
    }

    fn parse_constant_double_info(&mut self) -> Fallible<DoubleConstant> {
        let value = self.reader.read_f64::<BigEndian>()?;
        Ok(DoubleConstant { value })
    }

    fn parse_constant_name_and_type_info(&mut self) -> Fallible<NameAndTypeConstant> {
        let name_index = ConstantIndex::parse(&mut self.reader)?;
        let descriptor_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(NameAndTypeConstant {
            name_index,
            descriptor_index,
        })
    }

    fn parse_constant_utf8_info(&mut self) -> Fallible<Utf8Constant> {
        let len = self.reader.read_u16::<BigEndian>()?;
        let mut bytes = vec![0u8; len as usize];
        self.reader.read_exact(&mut bytes)?;
        Ok(Utf8Constant {
            string: from_java_cesu8(&bytes)?.into_owned(),
        })
    }

    fn parse_constant_method_handle_info(&mut self) -> Fallible<MethodHandleConstant> {
        let reference_kind = self.reader.read_u8()?;
        let reference_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(MethodHandleConstant {
            reference_kind,
            reference_index,
        })
    }

    fn parse_constant_method_type_info(&mut self) -> Fallible<MethodTypeConstant> {
        let descriptor_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(MethodTypeConstant { descriptor_index })
    }

    fn parse_constant_invoke_dynamic_info(&mut self) -> Fallible<InvokeDynamicConstant> {
        let bootstrap_method_attr_index = ConstantIndex::parse(&mut self.reader)?;
        let name_and_type_index = ConstantIndex::parse(&mut self.reader)?;
        Ok(InvokeDynamicConstant {
            bootstrap_method_attr_index,
            name_and_type_index,
        })
    }
}

#[derive(Debug)]
pub enum Constant {
    Class(ClassConstant),
    FieldRef(FieldRefConstant),
    MethodRef(MethodRefConstant),
    InterfaceMethodRef(InterfaceMethodRefConstant),
    String(StringConstant),
    Integer(IntegerConstant),
    Float(FloatConstant),
    Long(LongConstant),
    Double(DoubleConstant),
    NameAndType(NameAndTypeConstant),
    Utf8(Utf8Constant),
    MethodHandle(MethodHandleConstant),
    MethodType(MethodTypeConstant),
    InvokeDynamic(InvokeDynamicConstant),
    Unusable,
}

#[derive(Debug)]
pub struct ClassConstant {
    pub name_index: ConstantIndex,
}

#[derive(Debug)]
pub struct FieldRefConstant {
    pub class_index: ConstantIndex,
    pub name_and_type_index: ConstantIndex,
}

#[derive(Debug)]
pub struct MethodRefConstant {
    pub class_index: ConstantIndex,
    pub name_and_type_index: ConstantIndex,
}

#[derive(Debug)]
pub struct InterfaceMethodRefConstant {
    pub class_index: ConstantIndex,
    pub name_and_type_index: ConstantIndex,
}

#[derive(Debug)]
pub struct StringConstant {
    pub string_index: ConstantIndex,
}

#[derive(Debug)]
pub struct IntegerConstant {
    pub value: i32,
}

#[derive(Debug)]
pub struct FloatConstant {
    pub value: f32,
}

#[derive(Debug)]
pub struct LongConstant {
    pub value: i64,
}

#[derive(Debug)]
pub struct DoubleConstant {
    pub value: f64,
}

#[derive(Debug)]
pub struct NameAndTypeConstant {
    pub name_index: ConstantIndex,
    pub descriptor_index: ConstantIndex,
}

pub struct Utf8Constant {
    pub string: String,
}

impl fmt::Debug for Utf8Constant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.string, f)
    }
}

#[derive(Debug)]
pub struct MethodHandleConstant {
    pub reference_kind: u8,
    pub reference_index: ConstantIndex,
}

#[derive(Debug)]
pub struct MethodTypeConstant {
    pub descriptor_index: ConstantIndex,
}

#[derive(Debug)]
pub struct InvokeDynamicConstant {
    pub bootstrap_method_attr_index: ConstantIndex,
    pub name_and_type_index: ConstantIndex,
}
