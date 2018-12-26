use classfile::FieldType;
use strbuf::StrBuf;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    Object(StrBuf),
    Uninitialized,
    UninitializedThis,
}

impl Type {
    pub fn int() -> Self {
        Type::Integer
    }

    pub fn string() -> Self {
        Type::Object(StrBuf::new("java.lang.String"))
    }

    pub fn from_field_type(field_type: FieldType) -> Self {
        use classfile::descriptors::BaseType;

        match field_type {
            FieldType::Base(base_type) => match base_type {
                BaseType::Byte => Type::Integer,
                BaseType::Char => Type::Integer,
                BaseType::Short => Type::Integer,
                BaseType::Boolean => Type::Integer,
                BaseType::Int => Type::Integer,
                BaseType::Float => Type::Float,
                BaseType::Long => Type::Long,
                BaseType::Double => Type::Double,
            },
            FieldType::Object(object_type) => Type::Object(StrBuf::new(&object_type.class_name)),
            FieldType::Array(array_type) => {
                let class_name = format!("[{}", array_type.component_type.to_string());
                Type::Object(StrBuf::new(&class_name))
            }
        }
    }
}
