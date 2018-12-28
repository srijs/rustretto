use classfile::instructions::ArrayType;
use classfile::FieldType;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Boolean,
    Char,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    Reference,
}

impl Type {
    pub fn from_array_type(atype: &ArrayType) -> Type {
        match atype {
            ArrayType::Boolean => Type::Boolean,
            ArrayType::Char => Type::Char,
            ArrayType::Float => Type::Float,
            ArrayType::Double => Type::Double,
            ArrayType::Byte => Type::Byte,
            ArrayType::Short => Type::Short,
            ArrayType::Int => Type::Int,
            ArrayType::Long => Type::Long,
        }
    }

    pub fn from_field_type(field_type: &FieldType) -> Self {
        use classfile::descriptors::BaseType;

        match field_type {
            FieldType::Base(base_type) => match base_type {
                BaseType::Byte => Type::Byte,
                BaseType::Char => Type::Char,
                BaseType::Short => Type::Short,
                BaseType::Boolean => Type::Boolean,
                BaseType::Int => Type::Int,
                BaseType::Float => Type::Float,
                BaseType::Long => Type::Long,
                BaseType::Double => Type::Double,
            },
            FieldType::Object(_) => Type::Reference,
            FieldType::Array(_) => Type::Reference,
        }
    }

    pub fn can_unify_naive(&self, other: &Self) -> bool {
        self == other
    }
}
