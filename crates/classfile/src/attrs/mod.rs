use byteorder::{BigEndian, ReadBytesExt};
use failure::{bail, Fallible};

use super::{ConstantIndex, ConstantPool};
use crate::{ByteBuf, StrBuf};

pub mod code;
pub use self::code::Code;
pub mod stack_map_table;
pub use self::stack_map_table::StackMapTable;

#[derive(Clone, Debug)]
pub struct Attributes {
    attrs: Vec<(StrBuf, ByteBuf)>,
    consts: ConstantPool,
}

impl Attributes {
    pub(crate) fn parse(mut reader: &mut ByteBuf, consts: &ConstantPool) -> Fallible<Self> {
        let count = reader.read_u16::<BigEndian>()?;
        let mut attrs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name_index = ConstantIndex::parse(&mut reader)?;
            let name = consts.get_utf8(name_index).unwrap();
            let len = reader.read_u32::<BigEndian>()?;
            let info = reader.split_to(len as usize);
            attrs.push((name.0.clone(), info));
        }
        Ok(Attributes {
            attrs,
            consts: consts.clone(),
        })
    }

    pub fn get<A>(&self) -> Fallible<A>
    where
        A: Attribute,
    {
        if let Some(raw) = self.get_raw(A::NAME) {
            A::decode(raw, &self.consts)
        } else {
            bail!("attribute {} does not exist", A::NAME)
        }
    }

    pub fn get_raw(&self, name: &str) -> Option<RawAttribute> {
        self.attrs
            .iter()
            .find(|(s, _)| &**s == name)
            .map(|(_, bytes)| RawAttribute {
                bytes: bytes.clone(),
            })
    }
}

mod private {
    pub trait Sealed {}
}

pub trait Attribute: private::Sealed {
    const NAME: &'static str;

    fn decode(raw: RawAttribute, consts: &ConstantPool) -> Fallible<Self>
    where
        Self: Sized;
}

pub struct RawAttribute {
    pub(crate) bytes: ByteBuf,
}

impl AsRef<[u8]> for RawAttribute {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

#[derive(Debug)]
pub struct ConstantValue {
    pub value_index: ConstantIndex,
}

impl private::Sealed for ConstantValue {}

impl Attribute for ConstantValue {
    const NAME: &'static str = "ConstantValue";

    fn decode(raw: RawAttribute, _consts: &ConstantPool) -> Fallible<Self> {
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

impl Attribute for SourceFile {
    const NAME: &'static str = "SourceFile";

    fn decode(raw: RawAttribute, consts: &ConstantPool) -> Fallible<Self> {
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

impl Attribute for LineNumberTable {
    const NAME: &'static str = "LineNumberTable";

    fn decode(raw: RawAttribute, _consts: &ConstantPool) -> Fallible<Self> {
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
