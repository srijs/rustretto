use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use failure::Fallible;

use crate::ByteBuf;

#[derive(Clone, Debug)]
pub enum Instr {
    AaLoad,
    AaStore,
    AConstNull,
    ALoad(u8),
    ALoad0,
    ALoad1,
    ALoad2,
    ALoad3,
    ANewArray(u16),
    AReturn,
    ArrayLength,
    AStore(u8),
    AStore0,
    AStore1,
    AStore2,
    AStore3,
    AThrow,
    BaLoad,
    BaStore,
    BiPush(i8),
    CaLoad,
    CaStore,
    CheckCast(u16),
    D2F,
    D2I,
    D2L,
    DAdd,
    DaLoad,
    DaStore,
    DCmpG,
    DCmpL,
    DConst0,
    DConst1,
    DDiv,
    DLoad(u8),
    DMul,
    DNeg,
    DRem,
    DReturn,
    DStore(u8),
    DSub,
    Dup,
    DupX1,
    DupX2,
    Dup2,
    Dup2X1,
    Dup2X2,
    F2D,
    F2I,
    F2L,
    FAdd,
    FaLoad,
    FaStore,
    FCmpG,
    FCmpL,
    FConst0,
    FConst1,
    FConst2,
    FDiv,
    FLoad(u8),
    FMul,
    FNeg,
    FRem,
    FReturn,
    FStore(u8),
    FSub,
    GetField(u16),
    GetStatic(u16),
    Goto(i16),
    GotoW(i32),
    I2B,
    I2C,
    I2D,
    I2F,
    I2L,
    I2S,
    IAdd,
    IaLoad,
    IAnd,
    IaStore,
    IConstM1,
    IConst0,
    IConst1,
    IConst2,
    IConst3,
    IConst4,
    IConst5,
    IDiv,
    IfACmpEq(i16),
    IfACmpNe(i16),
    IfICmpEq(i16),
    IfICmpNe(i16),
    IfICmpLt(i16),
    IfICmpGe(i16),
    IfICmpGt(i16),
    IfICmpLe(i16),
    IfEq(i16),
    IfNe(i16),
    IfLt(i16),
    IfGe(i16),
    IfGt(i16),
    IfLe(i16),
    IfNonNull(i16),
    IfNull(i16),
    IInc(u8, i8),
    ILoad(u8),
    IMul,
    INeg,
    InstanceOf(u16),
    InvokeDynamic(u16, u16),
    InvokeInterface(u16, u8, u8),
    InvokeSpecial(u16),
    InvokeStatic(u16),
    InvokeVirtual(u16),
    IOr,
    IRem,
    IReturn,
    IShL,
    IShR,
    IStore(u8),
    ISub,
    IUShR,
    IXor,
    Jsr(i16),
    JsrW(i32),
    L2D,
    L2F,
    L2I,
    LAdd,
    LaLoad,
    LAnd,
    LaStore,
    LCmp,
    LConst0,
    LConst1,
    LdC(u8),
    LdCW(u16),
    LdC2W(u16),
    LDiv,
    LLoad(u8),
    LMul,
    LNeg,
    LookupSwitch(i32, Vec<(i32, i32)>),
    LOr,
    LRem,
    LReturn,
    LShL,
    LShR,
    LStore(u8),
    LSub,
    LUShR,
    LXor,
    MonitorEnter,
    MonitorExit,
    MultiNewArray(u16, u8),
    New(u16),
    NewArray(u8),
    Nop,
    Pop,
    Pop2,
    PutField(u16),
    PutStatic(u16),
    Ret(u8),
    Return,
    SaLoad,
    SaStore,
    SiPush(i16),
    Swap,
    TableSwitch(i32, i32, i32, Vec<i32>),
    WideILoad(u16),
    WideFLoad(u16),
    WideALoad(u16),
    WideLLoad(u16),
    WideDLoad(u16),
    WideIStore(u16),
    WideFStore(u16),
    WideAStore(u16),
    WideLStore(u16),
    WideDStore(u16),
    WideRet(u16),
    WideIInc(u16, i16),
}

impl Instr {
    pub fn may_throw_runtime_exception(&self) -> bool {
        match self {
            Instr::AaLoad => true,
            Instr::AaStore => true,
            Instr::ANewArray(_) => true,
            Instr::AReturn => true,
            Instr::ArrayLength => true,
            Instr::AThrow => true,
            Instr::BaLoad => true,
            Instr::BaStore => true,
            Instr::CaLoad => true,
            Instr::CaStore => true,
            Instr::CheckCast(_) => true,
            Instr::DaLoad => true,
            Instr::DaStore => true,
            Instr::DReturn => true,
            Instr::FaLoad => true,
            Instr::FaStore => true,
            Instr::FReturn => true,
            Instr::GetField(_) => true,
            Instr::GetStatic(_) => true,
            Instr::IaLoad => true,
            Instr::IaStore => true,
            Instr::IDiv => true,
            Instr::InvokeDynamic(_, _) => true,
            Instr::InvokeInterface(_, _, _) => true,
            Instr::InvokeSpecial(_) => true,
            Instr::InvokeStatic(_) => true,
            Instr::InvokeVirtual(_) => true,
            Instr::IRem => true,
            Instr::IReturn => true,
            Instr::LaLoad => true,
            Instr::LaStore => true,
            Instr::LDiv => true,
            Instr::LRem => true,
            Instr::LReturn => true,
            Instr::MonitorEnter => true,
            Instr::MonitorExit => true,
            Instr::MultiNewArray(_, _) => true,
            Instr::New(_) => true,
            Instr::NewArray(_) => true,
            Instr::PutField(_) => true,
            Instr::PutStatic(_) => true,
            Instr::Return => true,
            Instr::SaLoad => true,
            Instr::SaStore => true,
            _ => false,
        }
    }
}

pub struct Disassembler {
    code: Cursor<ByteBuf>,
}

impl Disassembler {
    pub(crate) fn new(code: ByteBuf) -> Self {
        Disassembler {
            code: Cursor::new(code),
        }
    }

    pub fn position(&self) -> u32 {
        self.code.position() as u32
    }

    pub fn set_position(&mut self, pos: u32) {
        self.code.set_position(pos as u64)
    }

    pub fn decode_next(&mut self) -> Fallible<Option<(u32, Instr)>> {
        let pos = self.position();
        if pos >= self.code.get_ref().len() as u32 {
            return Ok(None);
        }
        let instruction = match self.code.read_u8()? {
            0x32 => Instr::AaLoad,
            0x53 => Instr::AaStore,
            0x01 => Instr::AConstNull,
            0x19 => Instr::ALoad(self.code.read_u8()?),
            0x2a => Instr::ALoad0,
            0x2b => Instr::ALoad1,
            0x2c => Instr::ALoad2,
            0x2d => Instr::ALoad3,
            0xbd => Instr::ANewArray(self.code.read_u16::<BigEndian>()?),
            0xb0 => Instr::AReturn,
            0xbe => Instr::ArrayLength,
            0x3a => Instr::AStore(self.code.read_u8()?),
            0x4b => Instr::AStore0,
            0x4c => Instr::AStore1,
            0x4d => Instr::AStore2,
            0x4e => Instr::AStore3,
            0xbf => Instr::AThrow,
            0x33 => Instr::BaLoad,
            0x54 => Instr::BaStore,
            0x10 => Instr::BiPush(self.code.read_i8()?),
            0x34 => Instr::CaLoad,
            0x55 => Instr::CaStore,
            0xc0 => Instr::CheckCast(self.code.read_u16::<BigEndian>()?),
            0x90 => Instr::D2F,
            0x8e => Instr::D2I,
            0x8f => Instr::D2L,
            0x63 => Instr::DAdd,
            0x31 => Instr::DaLoad,
            0x52 => Instr::DaStore,
            0x98 => Instr::DCmpG,
            0x97 => Instr::DCmpL,
            0x0e => Instr::DConst0,
            0x0f => Instr::DConst1,
            0x6f => Instr::DDiv,
            0x18 => Instr::DLoad(self.code.read_u8()?),
            0x26 => Instr::DLoad(0),
            0x27 => Instr::DLoad(1),
            0x28 => Instr::DLoad(2),
            0x29 => Instr::DLoad(3),
            0x6b => Instr::DMul,
            0x77 => Instr::DNeg,
            0x73 => Instr::DRem,
            0xaf => Instr::DReturn,
            0x39 => Instr::DStore(self.code.read_u8()?),
            0x47 => Instr::DStore(0),
            0x48 => Instr::DStore(1),
            0x49 => Instr::DStore(2),
            0x4a => Instr::DStore(3),
            0x67 => Instr::DSub,
            0x59 => Instr::Dup,
            0x5a => Instr::DupX1,
            0x5b => Instr::DupX2,
            0x5c => Instr::Dup2,
            0x5d => Instr::Dup2X1,
            0x5e => Instr::Dup2X2,
            0x8d => Instr::F2D,
            0x8b => Instr::F2I,
            0x8c => Instr::F2L,
            0x62 => Instr::FAdd,
            0x30 => Instr::FaLoad,
            0x51 => Instr::FaStore,
            0x96 => Instr::FCmpG,
            0x95 => Instr::FCmpL,
            0x0b => Instr::FConst0,
            0x0c => Instr::FConst1,
            0x0d => Instr::FConst2,
            0x6e => Instr::FDiv,
            0x17 => Instr::FLoad(self.code.read_u8()?),
            0x22 => Instr::FLoad(0),
            0x23 => Instr::FLoad(1),
            0x24 => Instr::FLoad(2),
            0x25 => Instr::FLoad(3),
            0x6a => Instr::FMul,
            0x76 => Instr::FNeg,
            0x72 => Instr::FRem,
            0xae => Instr::FReturn,
            0x38 => Instr::FStore(self.code.read_u8()?),
            0x43 => Instr::FStore(0),
            0x44 => Instr::FStore(1),
            0x45 => Instr::FStore(2),
            0x46 => Instr::FStore(3),
            0x66 => Instr::FSub,
            0xb4 => Instr::GetField(self.code.read_u16::<BigEndian>()?),
            0xb2 => Instr::GetStatic(self.code.read_u16::<BigEndian>()?),
            0xa7 => Instr::Goto(self.code.read_i16::<BigEndian>()?),
            0xc8 => Instr::GotoW(self.code.read_i32::<BigEndian>()?),
            0x91 => Instr::I2B,
            0x92 => Instr::I2C,
            0x87 => Instr::I2D,
            0x86 => Instr::I2F,
            0x85 => Instr::I2L,
            0x93 => Instr::I2S,
            0x60 => Instr::IAdd,
            0x2e => Instr::IaLoad,
            0x7e => Instr::IAnd,
            0x4f => Instr::IaStore,
            0x02 => Instr::IConstM1,
            0x03 => Instr::IConst0,
            0x04 => Instr::IConst1,
            0x05 => Instr::IConst2,
            0x06 => Instr::IConst3,
            0x07 => Instr::IConst4,
            0x08 => Instr::IConst5,
            0x6c => Instr::IDiv,
            0xa5 => Instr::IfACmpEq(self.code.read_i16::<BigEndian>()?),
            0xa6 => Instr::IfACmpNe(self.code.read_i16::<BigEndian>()?),
            0x9f => Instr::IfICmpEq(self.code.read_i16::<BigEndian>()?),
            0xa0 => Instr::IfICmpNe(self.code.read_i16::<BigEndian>()?),
            0xa1 => Instr::IfICmpLt(self.code.read_i16::<BigEndian>()?),
            0xa2 => Instr::IfICmpGe(self.code.read_i16::<BigEndian>()?),
            0xa3 => Instr::IfICmpGt(self.code.read_i16::<BigEndian>()?),
            0xa4 => Instr::IfICmpLe(self.code.read_i16::<BigEndian>()?),
            0x99 => Instr::IfEq(self.code.read_i16::<BigEndian>()?),
            0x9a => Instr::IfNe(self.code.read_i16::<BigEndian>()?),
            0x9b => Instr::IfLt(self.code.read_i16::<BigEndian>()?),
            0x9c => Instr::IfGe(self.code.read_i16::<BigEndian>()?),
            0x9d => Instr::IfGt(self.code.read_i16::<BigEndian>()?),
            0x9e => Instr::IfLe(self.code.read_i16::<BigEndian>()?),
            0xc7 => Instr::IfNonNull(self.code.read_i16::<BigEndian>()?),
            0xc6 => Instr::IfNull(self.code.read_i16::<BigEndian>()?),
            0x84 => Instr::IInc(self.code.read_u8()?, self.code.read_i8()?),
            0x15 => Instr::ILoad(self.code.read_u8()?),
            0x1a => Instr::ILoad(0),
            0x1b => Instr::ILoad(1),
            0x1c => Instr::ILoad(2),
            0x1d => Instr::ILoad(3),
            0x68 => Instr::IMul,
            0x74 => Instr::INeg,
            0xc1 => Instr::InstanceOf(self.code.read_u16::<BigEndian>()?),
            0xba => Instr::InvokeDynamic(
                self.code.read_u16::<BigEndian>()?,
                self.code.read_u16::<BigEndian>()?,
            ),
            0xb9 => Instr::InvokeInterface(
                self.code.read_u16::<BigEndian>()?,
                self.code.read_u8()?,
                self.code.read_u8()?,
            ),
            0xb7 => Instr::InvokeSpecial(self.code.read_u16::<BigEndian>()?),
            0xb8 => Instr::InvokeStatic(self.code.read_u16::<BigEndian>()?),
            0xb6 => Instr::InvokeVirtual(self.code.read_u16::<BigEndian>()?),
            0x80 => Instr::IOr,
            0x70 => Instr::IRem,
            0xac => Instr::IRem,
            0x78 => Instr::IShL,
            0x7a => Instr::IShR,
            0x36 => Instr::IStore(self.code.read_u8()?),
            0x3b => Instr::IStore(0),
            0x3c => Instr::IStore(1),
            0x3d => Instr::IStore(2),
            0x3e => Instr::IStore(3),
            0x64 => Instr::ISub,
            0x7c => Instr::IUShR,
            0x82 => Instr::IXor,
            0xa8 => Instr::Jsr(self.code.read_i16::<BigEndian>()?),
            0xc9 => Instr::JsrW(self.code.read_i32::<BigEndian>()?),
            0x8a => Instr::L2D,
            0x89 => Instr::L2F,
            0x88 => Instr::L2I,
            0x61 => Instr::LAdd,
            0x2f => Instr::LaLoad,
            0x7f => Instr::LAnd,
            0x50 => Instr::LaStore,
            0x94 => Instr::LCmp,
            0x09 => Instr::LConst0,
            0x0a => Instr::LConst1,
            0x12 => Instr::LdC(self.code.read_u8()?),
            0x13 => Instr::LdCW(self.code.read_u16::<BigEndian>()?),
            0x14 => Instr::LdC2W(self.code.read_u16::<BigEndian>()?),
            0x6d => Instr::LDiv,
            0x16 => Instr::LLoad(self.code.read_u8()?),
            0x1e => Instr::LLoad(0),
            0x1f => Instr::LLoad(1),
            0x20 => Instr::LLoad(2),
            0x21 => Instr::LLoad(3),
            0x69 => Instr::LMul,
            0x75 => Instr::LNeg,
            0xab => unimplemented!("TODO: decode lookupswitch"),
            0x81 => Instr::LOr,
            0x71 => Instr::LRem,
            0xad => Instr::LReturn,
            0x79 => Instr::LShL,
            0x7b => Instr::LShR,
            0x37 => Instr::LStore(self.code.read_u8()?),
            0x3f => Instr::LStore(0),
            0x40 => Instr::LStore(1),
            0x41 => Instr::LStore(2),
            0x42 => Instr::LStore(3),
            0x65 => Instr::LSub,
            0x7d => Instr::LUShR,
            0x83 => Instr::LXor,
            0xc2 => Instr::MonitorEnter,
            0xc3 => Instr::MonitorExit,
            0xc5 => unimplemented!("TODO: decode multianewarray"),
            0xbb => Instr::New(self.code.read_u16::<BigEndian>()?),
            0xbc => Instr::NewArray(self.code.read_u8()?),
            0x00 => Instr::Nop,
            0x57 => Instr::Pop,
            0x58 => Instr::Pop2,
            0xb5 => Instr::PutField(self.code.read_u16::<BigEndian>()?),
            0xb3 => Instr::PutStatic(self.code.read_u16::<BigEndian>()?),
            0xa9 => Instr::Ret(self.code.read_u8()?),
            0xb1 => Instr::Return,
            0x35 => Instr::SaLoad,
            0x56 => Instr::SaStore,
            0x11 => Instr::SiPush(self.code.read_i16::<BigEndian>()?),
            0x5f => Instr::Swap,
            0xaa => unimplemented!("TODO: decode tableswitch"),
            0xc4 => match self.code.read_u8()? {
                0x15 => Instr::WideILoad(self.code.read_u16::<BigEndian>()?),
                0x17 => Instr::WideFLoad(self.code.read_u16::<BigEndian>()?),
                0x19 => Instr::WideALoad(self.code.read_u16::<BigEndian>()?),
                0x16 => Instr::WideLLoad(self.code.read_u16::<BigEndian>()?),
                0x18 => Instr::WideDLoad(self.code.read_u16::<BigEndian>()?),
                0x36 => Instr::WideIStore(self.code.read_u16::<BigEndian>()?),
                0x38 => Instr::WideFStore(self.code.read_u16::<BigEndian>()?),
                0x3a => Instr::WideAStore(self.code.read_u16::<BigEndian>()?),
                0x37 => Instr::WideLStore(self.code.read_u16::<BigEndian>()?),
                0x39 => Instr::WideDStore(self.code.read_u16::<BigEndian>()?),
                0xa9 => Instr::WideRet(self.code.read_u16::<BigEndian>()?),
                0x84 => Instr::WideIInc(
                    self.code.read_u16::<BigEndian>()?,
                    self.code.read_i16::<BigEndian>()?,
                ),
                unknown_opcode => bail!("unknown wide opcode {:x}", unknown_opcode),
            },
            unknown_opcode => bail!("unknown opcode {:x}", unknown_opcode),
        };
        Ok(Some((pos, instruction)))
    }
}
