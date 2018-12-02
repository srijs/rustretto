use classfile::constant_pool::Utf8Constant;
use classfile::FieldType;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    Object(Utf8Constant),
    Uninitialized,
    UninitializedThis,
}

impl Type {
    pub fn int() -> Self {
        Type::Integer
    }

    pub fn string() -> Self {
        Type::Object(Utf8Constant::from_str("java.lang.String"))
    }

    pub fn from_field_type(field_type: FieldType) -> Self {
        match field_type {
            FieldType::Base(base_type) => match base_type {
                classfile::descriptors::BaseType::Boolean => Self::int(),
                _ => unimplemented!("unsupported base type {:?}", base_type),
            },
            FieldType::Object(object_type) => {
                Type::Object(Utf8Constant::from_str(&object_type.class_name))
            }
            FieldType::Array(array_type) => {
                let class_name = format!("[{}", array_type.component_type.to_string());
                Type::Object(Utf8Constant::from_str(&class_name))
            }
        }
    }
}
