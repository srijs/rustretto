use std::collections::HashMap;
use std::io::Read;

use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::{ConstantIndex, ConstantPool};

pub mod code;
pub use self::code::Code;
pub mod stack_map_table;
pub use self::stack_map_table::StackMapTable;

#[derive(Debug)]
pub struct Attributes {
    attrs: HashMap<String, Vec<u8>>,
    consts: ConstantPool,
}

impl Attributes {
    pub(crate) fn parse<R: Read>(mut reader: R, consts: &ConstantPool) -> Fallible<Self> {
        let count = reader.read_u16::<BigEndian>()?;
        let mut attrs = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let name_index = ConstantIndex::parse(&mut reader)?;
            let name = consts.get_utf8(name_index).unwrap();
            let len = reader.read_u32::<BigEndian>()?;
            let mut info = vec![0u8; len as usize];
            reader.read_exact(&mut info)?;
            attrs.insert(name.into(), info);
        }
        Ok(Attributes {
            attrs,
            consts: consts.clone(),
        })
    }

    pub fn get<'a, A>(&'a self) -> Fallible<A>
    where
        A: Attribute<'a>,
    {
        if let Some(raw) = self.get_raw(A::NAME) {
            A::decode(raw, &self.consts)
        } else {
            bail!("attribute {} does not exist", A::NAME)
        }
    }

    pub fn get_raw(&self, name: &str) -> Option<RawAttribute> {
        self.attrs
            .get(name)
            .map(|bytes| RawAttribute { bytes: &bytes })
    }
}

mod private {
    pub trait Sealed {}
}

pub trait Attribute<'a>: private::Sealed {
    const NAME: &'static str;

    fn decode(raw: RawAttribute<'a>, consts: &ConstantPool) -> Fallible<Self>
    where
        Self: Sized;
}

pub struct RawAttribute<'a> {
    bytes: &'a [u8],
}

impl<'a> AsRef<[u8]> for RawAttribute<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug)]
pub struct ConstantValue {
    pub value_index: ConstantIndex,
}

impl private::Sealed for ConstantValue {}

impl<'a> Attribute<'a> for ConstantValue {
    const NAME: &'static str = "ConstantValue";

    fn decode(raw: RawAttribute<'a>, _consts: &ConstantPool) -> Fallible<Self> {
        let value_index = ConstantIndex::parse(raw.as_ref())?;
        Ok(ConstantValue { value_index })
    }
}

#[derive(Debug)]
pub struct SourceFile {
    index: ConstantIndex,
    consts: ConstantPool,
}

impl SourceFile {
    pub fn index(&self) -> ConstantIndex {
        self.index
    }

    pub fn as_str(&self) -> &str {
        self.consts.get_utf8(self.index).unwrap()
    }
}

impl private::Sealed for SourceFile {}

impl<'a> Attribute<'a> for SourceFile {
    const NAME: &'static str = "SourceFile";

    fn decode(raw: RawAttribute<'a>, consts: &ConstantPool) -> Fallible<Self> {
        let index = ConstantIndex::parse(raw.as_ref())?;
        Ok(SourceFile {
            index,
            consts: consts.clone(),
        })
    }
}

#[derive(Debug)]
pub struct LineNumberTable {
    pub entries: Vec<LineNumberTableEntry>,
}

impl private::Sealed for LineNumberTable {}

impl<'a> Attribute<'a> for LineNumberTable {
    const NAME: &'static str = "LineNumberTable";

    fn decode(raw: RawAttribute<'a>, _consts: &ConstantPool) -> Fallible<Self> {
        let mut bytes = raw.as_ref();
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
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16,
}
