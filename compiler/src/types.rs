use classfile::FieldType;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    Object(String),
    Uninitialized,
    UninitializedThis,
}

impl Type {
    pub fn int() -> Self {
        Type::Integer
    }

    pub fn string() -> Self {
        Type::Object("java.lang.String".to_owned())
    }

    pub fn from_field_type(field_type: FieldType) -> Self {
        match field_type {
            FieldType::Base(base_type) => match base_type {
                classfile::descriptors::BaseType::Boolean => Self::int(),
                _ => unimplemented!("unsupported base type {:?}", base_type),
            },
            FieldType::Object(object_type) => Type::Object(object_type.class_name),
            FieldType::Array(array_type) => {
                let class_name = format!("[{}", array_type.component_type.to_string());
                Type::Object(class_name)
            }
        }
    }
}
