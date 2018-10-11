use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use super::super::{ConstantIndex, ConstantPool};
use super::{private, Attribute, RawAttribute};

#[derive(Debug)]
pub struct StackMapTable<'a> {
    count: u16,
    bytes: &'a [u8],
}

impl<'a> StackMapTable<'a> {
    pub fn len(&self) -> u16 {
        self.count
    }

    pub fn entries(&self) -> Entries<'a> {
        Entries {
            count: self.count,
            bytes: self.bytes,
        }
    }
}

impl<'a> private::Sealed for StackMapTable<'a> {}

impl<'a> Attribute<'a> for StackMapTable<'a> {
    const NAME: &'static str = "StackMapTable";

    fn decode(raw: RawAttribute<'a>, _consts: &ConstantPool) -> Fallible<Self> {
        let mut bytes = raw.bytes.as_ref();
        let count = bytes.read_u16::<BigEndian>()?;
        Ok(StackMapTable { count, bytes })
    }
}

#[derive(Debug)]
pub struct Entries<'a> {
    count: u16,
    bytes: &'a [u8],
}

impl<'a> Iterator for Entries<'a> {
    type Item = Fallible<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1;
        Some(parse_stack_map_table_entry(&mut self.bytes))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.count as usize))
    }
}

#[derive(Debug)]
pub enum Entry {
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

fn parse_stack_map_table_entry(mut bytes: &[u8]) -> Fallible<Entry> {
    let frame_type = bytes.read_u8()?;
    if frame_type <= 63 {
        Ok(Entry::SameFrame {
            offset_delta: frame_type,
        })
    } else if frame_type >= 64 && frame_type <= 127 {
        let stack_item = parse_verification_type_info(bytes)?;
        Ok(Entry::SameLocals1StackItem {
            offset_delta: frame_type - 64,
            stack_item,
        })
    } else if frame_type == 247 {
        let offset_delta = bytes.read_u16::<BigEndian>()?;
        let stack_item = parse_verification_type_info(bytes)?;
        Ok(Entry::SameLocals1StackItemExtended {
            offset_delta,
            stack_item,
        })
    } else if frame_type >= 248 && frame_type <= 250 {
        let offset_delta = bytes.read_u16::<BigEndian>()?;
        Ok(Entry::ChopFrame {
            offset_delta,
            k: 251 - frame_type,
        })
    } else if frame_type == 251 {
        let offset_delta = bytes.read_u16::<BigEndian>()?;
        Ok(Entry::SameFrameExtended { offset_delta })
    } else if frame_type >= 252 && frame_type <= 254 {
        let offset_delta = bytes.read_u16::<BigEndian>()?;
        let k = frame_type - 251;
        let mut locals = Vec::with_capacity(k as usize);
        for _ in 0..k {
            locals.push(parse_verification_type_info(&mut bytes)?);
        }
        Ok(Entry::AppendFrame {
            offset_delta,
            locals,
        })
    } else if frame_type == 255 {
        let offset_delta = bytes.read_u16::<BigEndian>()?;
        let number_of_locals = bytes.read_u16::<BigEndian>()?;
        let mut locals = Vec::with_capacity(number_of_locals as usize);
        for _ in 0..number_of_locals {
            locals.push(parse_verification_type_info(&mut bytes)?);
        }
        let number_of_stack_items = bytes.read_u16::<BigEndian>()?;
        let mut stack_items = Vec::with_capacity(number_of_stack_items as usize);
        for _ in 0..number_of_stack_items {
            stack_items.push(parse_verification_type_info(&mut bytes)?);
        }
        Ok(Entry::FullFrame {
            offset_delta,
            locals,
            stack_items,
        })
    } else {
        bail!("unknown frame type {}", frame_type)
    }
}
