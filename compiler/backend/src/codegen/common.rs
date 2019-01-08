use std::fmt;

use classfile::descriptors::{
    BaseType, FieldType, MethodDescriptor, ParameterDescriptor, ReturnTypeDescriptor,
};

use frontend::loader::ArrayClass;
use frontend::translate::{Const, Op, VarId};
use frontend::types::Type;

pub enum Dest {
    Ignore,
    Assign(DestAssign),
}

pub enum DestAssign {
    Var(VarId),
    Tmp(u64),
}

impl fmt::Display for DestAssign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DestAssign::Var(v) => write!(f, "%v{}", v.1),
            DestAssign::Tmp(t) => write!(f, "%t{}", t),
        }
    }
}

pub struct TmpVarIdGen {
    next_id: u64,
}

impl TmpVarIdGen {
    pub fn new() -> Self {
        TmpVarIdGen { next_id: 0 }
    }

    pub fn gen(&mut self) -> u64 {
        let var_id = self.next_id;
        self.next_id += 1;
        var_id
    }
}

pub struct OpVal<'a>(pub &'a Op);

impl<'a> fmt::Display for OpVal<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Op::Var(v) => write!(f, "%v{}", v.1),
            Op::Const(c) => match c {
                Const::Int(i) => write!(f, "{}", i),
                Const::Long(j) => write!(f, "{}", j),
                Const::Null => write!(f, "zeroinitializer"),
            },
        }
    }
}

pub struct GenFunctionType<'a>(pub &'a MethodDescriptor);

impl<'a> fmt::Display for GenFunctionType<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(tlt_return_type(&self.0.ret))?;
        f.write_str(" (%ref")?;
        for ParameterDescriptor::Field(field) in self.0.params.iter() {
            f.write_str(", ")?;
            f.write_str(tlt_field_type(field))?;
        }
        f.write_str(")")?;
        Ok(())
    }
}

pub fn tlt_return_type(return_type: &ReturnTypeDescriptor) -> &'static str {
    match return_type {
        ReturnTypeDescriptor::Void => "void",
        ReturnTypeDescriptor::Field(field_type) => tlt_field_type(field_type),
    }
}

pub fn tlt_field_type(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Base(base_type) => match base_type {
            BaseType::Boolean => "i32",
            BaseType::Byte => "i32",
            BaseType::Char => "i32",
            BaseType::Short => "i32",
            BaseType::Int => "i32",
            BaseType::Long => "i64",
            BaseType::Float => "float",
            BaseType::Double => "double",
        },
        FieldType::Object(_) | FieldType::Array(_) => "%ref",
    }
}

pub fn tlt_array_class_component_type(array_class: &ArrayClass) -> &'static str {
    match array_class {
        ArrayClass::Complex(_) => "%ref",
        ArrayClass::Primitive(base_type) => match base_type {
            BaseType::Boolean => "i8",
            BaseType::Byte => "i8",
            BaseType::Char => "i8",
            BaseType::Short => "i16",
            BaseType::Int => "i32",
            BaseType::Long => "i64",
            BaseType::Float => "float",
            BaseType::Double => "double",
        },
    }
}

pub fn tlt_array_component_type(ctyp: &Type) -> &'static str {
    match ctyp {
        Type::Boolean => "i8",
        Type::Byte => "i8",
        Type::Char => "i8",
        Type::Short => "i16",
        Type::Int => "i32",
        Type::Long => "i64",
        Type::Float => "float",
        Type::Double => "double",
        Type::Reference => "%ref",
    }
}

pub fn tlt_type(t: &Type) -> &'static str {
    match t {
        Type::Boolean => "i32",
        Type::Byte => "i32",
        Type::Char => "i32",
        Type::Short => "i32",
        Type::Int => "i32",
        Type::Long => "i64",
        Type::Float => "float",
        Type::Double => "double",
        Type::Reference => "%ref",
    }
}
