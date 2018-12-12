use std::ffi::CString;
use std::ptr;

use libc::c_char;
use llvm_sys::target::*;
use llvm_sys::target_machine::*;

use crate::error::Error;
use crate::message::Message;

pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}

pub struct TargetMachineBuilder {
    triple: CString,
    level: LLVMCodeGenOptLevel,
    reloc: LLVMRelocMode,
    code_model: LLVMCodeModel,
}

impl TargetMachineBuilder {
    pub fn new(triple: &str) -> Self {
        let triple_cstring = CString::new(triple).unwrap();

        let level = LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault;
        let reloc = LLVMRelocMode::LLVMRelocDefault;
        let code_model = LLVMCodeModel::LLVMCodeModelDefault;

        TargetMachineBuilder {
            triple: triple_cstring,
            level,
            reloc,
            code_model,
        }
    }

    pub fn optimize(&mut self, level: OptLevel) {
        self.level = match level {
            OptLevel::None => LLVMCodeGenOptLevel::LLVMCodeGenLevelNone,
            OptLevel::Less => LLVMCodeGenOptLevel::LLVMCodeGenLevelLess,
            OptLevel::Default => LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
            OptLevel::Aggressive => LLVMCodeGenOptLevel::LLVMCodeGenLevelAggressive,
        }
    }

    pub fn build(self) -> Result<TargetMachine, Error> {
        let llref;
        unsafe {
            let mut target = ptr::null_mut();
            let mut msg_ptr = ptr::null_mut();
            let code = LLVMGetTargetFromTriple(
                self.triple.as_ptr() as *const c_char,
                &mut target as *mut LLVMTargetRef,
                &mut msg_ptr as *mut *mut c_char,
            );
            if code != 0 {
                return Err(Error::from_ptr(msg_ptr));
            }
            llref = LLVMCreateTargetMachine(
                target,
                self.triple.as_ptr() as *const c_char,
                b"\0".as_ptr() as *const c_char,
                b"\0".as_ptr() as *const c_char,
                self.level,
                self.reloc,
                self.code_model,
            );
        }
        Ok(TargetMachine { llref })
    }
}

pub struct TargetMachine {
    llref: LLVMTargetMachineRef,
}

pub enum FileType {
    Assembly,
    Object,
}

impl TargetMachine {
    pub fn builder(triple: &str) -> TargetMachineBuilder {
        TargetMachineBuilder::new(triple)
    }

    pub fn data_layout(&self) -> TargetDataLayout {
        let llref;
        unsafe {
            llref = LLVMCreateTargetDataLayout(self.llref);
        }
        TargetDataLayout { llref }
    }
}

impl Drop for TargetMachine {
    fn drop(&mut self) {
        unsafe { LLVMDisposeTargetMachine(self.llref) }
    }
}

pub struct TargetDataLayout {
    llref: LLVMTargetDataRef,
}

impl TargetDataLayout {
    pub fn to_string_rep(&self) -> Message {
        let ptr;
        unsafe {
            ptr = LLVMCopyStringRepOfTargetData(self.llref);
        }
        Message { inner: ptr }
    }
}

impl Drop for TargetDataLayout {
    fn drop(&mut self) {
        unsafe { LLVMDisposeTargetData(self.llref) }
    }
}
