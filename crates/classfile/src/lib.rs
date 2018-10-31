#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate bytes;
extern crate cesu8;
#[macro_use]
extern crate failure;

use std::io::Read;

use byteorder::{BigEndian, NativeEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use failure::Fallible;

mod access_flags;
pub use self::access_flags::{ClassAccessFlags, FieldAccessFlags, MethodAccessFlags};
pub mod constant_pool;
pub use self::constant_pool::{ConstantIndex, ConstantPool};
pub mod attrs;
pub use self::attrs::{Attribute, Attributes};
pub mod descriptors;
pub use self::descriptors::{FieldType, MethodDescriptor};
pub mod instructions;

#[derive(Debug)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

#[derive(Debug)]
pub struct Field {
    pub access_flags: FieldAccessFlags,
    pub name_index: ConstantIndex,
    pub descriptor_index: ConstantIndex,
    pub attributes: Attributes,
}

#[derive(Debug)]
pub struct Method {
    pub access_flags: MethodAccessFlags,
    pub name_index: ConstantIndex,
    pub descriptor: MethodDescriptor,
    pub attributes: Attributes,
}

#[derive(Debug)]
pub struct MethodRef {
    pub class_index: ConstantIndex,
    pub name_index: ConstantIndex,
    pub descriptor: MethodDescriptor,
}

#[derive(Debug)]
pub struct FieldRef {
    pub class_index: ConstantIndex,
    pub name_index: ConstantIndex,
    pub descriptor: FieldType,
}

#[derive(Debug)]
pub struct ClassFile {
    pub version: Version,
    pub constant_pool: ConstantPool,
    pub access_flags: ClassAccessFlags,
    pub this_class: ConstantIndex,
    pub super_class: Option<ConstantIndex>,
    pub interfaces: Vec<ConstantIndex>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub attributes: Attributes,
}

impl ClassFile {
    pub fn parse<R: Read>(reader: R) -> Fallible<Self> {
        let mut parser = ClassFileParser::new(reader);

        parser.parse_magic()?;
        let version = parser.parse_version()?;
        let constant_pool = parser.parse_constant_pool()?;
        let access_flags = parser.parse_access_flags()?;
        let this_class = parser.parse_this_class()?;
        let super_class = parser.parse_super_class()?;
        let interfaces = parser.parse_interfaces()?;
        let fields = parser.parse_fields(&constant_pool)?;
        let methods = parser.parse_methods(&constant_pool)?;
        let attributes = parser.parse_attributes(&constant_pool)?;

        Ok(ClassFile {
            version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            interfaces,
            fields,
            methods,
            attributes,
        })
    }

    pub fn get_this_class(&self) -> &self::constant_pool::ClassConstant {
        self.constant_pool.get_class(self.this_class).unwrap()
    }

    pub fn get_super_class(&self) -> Option<&self::constant_pool::ClassConstant> {
        self.super_class
            .map(|idx| self.constant_pool.get_class(idx).unwrap())
    }
}

struct ClassFileParser<R> {
    reader: R,
}

impl<R: Read> ClassFileParser<R> {
    fn new(reader: R) -> Self {
        ClassFileParser { reader }
    }

    fn parse_magic(&mut self) -> Fallible<()> {
        let magic = self.reader.read_u32::<NativeEndian>()?;
        ensure!(magic != 0xCAFEBABE, "unknown magic byte sequence");
        Ok(())
    }

    fn parse_version(&mut self) -> Fallible<Version> {
        let minor = self.reader.read_u16::<BigEndian>()?;
        let major = self.reader.read_u16::<BigEndian>()?;
        Ok(Version { major, minor })
    }

    fn parse_constant_pool(&mut self) -> Fallible<ConstantPool> {
        ConstantPool::parse(&mut self.reader)
    }

    fn parse_access_flags(&mut self) -> Fallible<ClassAccessFlags> {
        let bits = self.reader.read_u16::<BigEndian>()?;
        Ok(ClassAccessFlags::from_bits_truncate(bits))
    }

    fn parse_this_class(&mut self) -> Fallible<ConstantIndex> {
        ConstantIndex::parse(&mut self.reader)
    }

    fn parse_super_class(&mut self) -> Fallible<Option<ConstantIndex>> {
        let idx = self.reader.read_u16::<BigEndian>()?;
        if idx > 0 {
            Ok(Some(ConstantIndex(idx)))
        } else {
            Ok(None)
        }
    }

    fn parse_interfaces(&mut self) -> Fallible<Vec<ConstantIndex>> {
        let count = self.reader.read_u16::<BigEndian>()?;
        let mut interfaces = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let idx = self.reader.read_u16::<BigEndian>()?;
            interfaces.push(ConstantIndex(idx));
        }
        Ok(interfaces)
    }

    fn parse_fields(&mut self, constants: &ConstantPool) -> Fallible<Vec<Field>> {
        let count = self.reader.read_u16::<BigEndian>()?;
        let mut fields = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let access_flags_bits = self.reader.read_u16::<BigEndian>()?;
            let access_flags = FieldAccessFlags::from_bits_truncate(access_flags_bits);
            let name_index = ConstantIndex::parse(&mut self.reader)?;
            let descriptor_index = ConstantIndex::parse(&mut self.reader)?;
            let attributes = Attributes::parse(&mut self.reader, constants)?;
            fields.push(Field {
                access_flags,
                name_index,
                descriptor_index,
                attributes,
            })
        }
        Ok(fields)
    }

    fn parse_methods(&mut self, constants: &ConstantPool) -> Fallible<Vec<Method>> {
        let count = self.reader.read_u16::<BigEndian>()?;
        let mut methods = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let access_flags_bits = self.reader.read_u16::<BigEndian>()?;
            let access_flags = MethodAccessFlags::from_bits_truncate(access_flags_bits);
            let name_index = ConstantIndex::parse(&mut self.reader)?;
            let descriptor_index = ConstantIndex::parse(&mut self.reader)?;
            let descriptor_string = constants.get_utf8(descriptor_index).unwrap();
            let descriptor = MethodDescriptor::parse(descriptor_string.as_bytes())?;
            let attributes = Attributes::parse(&mut self.reader, constants)?;
            methods.push(Method {
                access_flags,
                name_index,
                descriptor,
                attributes,
            })
        }
        Ok(methods)
    }

    fn parse_attributes(&mut self, constants: &ConstantPool) -> Fallible<Attributes> {
        Attributes::parse(&mut self.reader, constants)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ByteBuf(Bytes);

impl ByteBuf {
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn split_to(&mut self, at: usize) -> ByteBuf {
        ByteBuf(self.0.split_to(at))
    }
}

impl Buf for ByteBuf {
    fn remaining(&self) -> usize {
        self.0.len()
    }

    fn bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    fn advance(&mut self, cnt: usize) {
        self.0.advance(cnt)
    }
}

impl AsRef<[u8]> for ByteBuf {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Read for ByteBuf {
    fn read(&mut self, buf: &mut [u8]) -> ::std::io::Result<usize> {
        self.reader().read(buf)
    }
}

impl From<Vec<u8>> for ByteBuf {
    fn from(vec: Vec<u8>) -> Self {
        ByteBuf(vec.into())
    }
}
