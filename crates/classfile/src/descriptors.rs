use std::io::BufRead;

use failure::Fallible;

#[derive(Debug)]
pub struct MethodDescriptor {
    pub params: Vec<ParameterDescriptor>,
    pub ret: ReturnTypeDescriptor,
}

impl MethodDescriptor {
    pub(crate) fn parse<R: BufRead>(mut reader: R) -> Fallible<Self> {
        let mut tag = [0u8; 1];
        reader.read_exact(&mut tag)?;
        ensure!(tag[0] == '(' as u8, "expected parameter descriptors");
        let mut params = Vec::new();
        loop {
            reader.read_exact(&mut tag)?;
            if tag[0] == ')' as u8 {
                break;
            }
            let field_type = FieldType::parse_with_tag(&mut reader, tag[0])?;
            params.push(ParameterDescriptor::Field(field_type));
        }
        reader.read_exact(&mut tag)?;
        let ret = if tag[0] == 'V' as u8 {
            ReturnTypeDescriptor::Void
        } else {
            ReturnTypeDescriptor::Field(FieldType::parse_with_tag(reader, tag[0])?)
        };
        Ok(MethodDescriptor { params, ret })
    }
}

#[derive(Clone, Debug)]
pub enum ParameterDescriptor {
    Field(FieldType),
}

#[derive(Clone, Debug)]
pub enum ReturnTypeDescriptor {
    Field(FieldType),
    Void,
}

#[derive(Clone, Debug)]
pub enum FieldType {
    Base(BaseType),
    Object(ObjectType),
    Array(ArrayType),
}

impl FieldType {
    pub(crate) fn parse<R: BufRead>(mut reader: R) -> Fallible<Self> {
        let mut tag = [0u8; 1];
        reader.read_exact(&mut tag)?;
        FieldType::parse_with_tag(reader, tag[0])
    }

    pub(crate) fn parse_with_tag<R: BufRead>(mut reader: R, tag: u8) -> Fallible<Self> {
        match tag as char {
            'B' => Ok(FieldType::Base(BaseType::Byte)),
            'C' => Ok(FieldType::Base(BaseType::Char)),
            'D' => Ok(FieldType::Base(BaseType::Double)),
            'F' => Ok(FieldType::Base(BaseType::Float)),
            'I' => Ok(FieldType::Base(BaseType::Int)),
            'J' => Ok(FieldType::Base(BaseType::Long)),
            'S' => Ok(FieldType::Base(BaseType::Short)),
            'Z' => Ok(FieldType::Base(BaseType::Boolean)),
            'L' => {
                let mut class_name_bytes = Vec::new();
                reader.read_until(';' as u8, &mut class_name_bytes)?;
                if class_name_bytes.pop() != Some(';' as u8) {
                    bail!("invalid class name");
                }
                let class_name = String::from_utf8(class_name_bytes)?.replace('/', ".");
                Ok(FieldType::Object(ObjectType { class_name }))
            }
            '[' => {
                let component_type = Box::new(FieldType::parse(reader)?);
                Ok(FieldType::Array(ArrayType { component_type }))
            }
            _ => bail!("unknown descriptor tag {}", tag),
        }
    }

    pub fn try_from_str(input: &str) -> Fallible<Self> {
        Self::parse(input.as_bytes())
    }

    pub fn to_string(&self) -> String {
        let mut output = String::new();
        let mut field_type = self;
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
}

#[derive(Clone, Debug)]
pub enum BaseType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
}

#[derive(Clone, Debug)]
pub struct ObjectType {
    pub class_name: String,
}

#[derive(Clone, Debug)]
pub struct ArrayType {
    pub component_type: Box<FieldType>,
}
