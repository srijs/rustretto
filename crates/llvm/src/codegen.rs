use std::ptr;
use std::sync::Once;

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

static INIT_NATIVE_TARGET: Once = Once::new();

pub struct TargetMachineBuilder {
    level: LLVMCodeGenOptLevel,
    reloc: LLVMRelocMode,
    code_model: LLVMCodeModel,
}

impl TargetMachineBuilder {
    pub fn new() -> Self {
        let level = LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault;
        let reloc = LLVMRelocMode::LLVMRelocDefault;
        let code_model = LLVMCodeModel::LLVMCodeModelDefault;

        TargetMachineBuilder {
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
        INIT_NATIVE_TARGET.call_once(|| {
            let code;
            unsafe {
                code = llvm_sys::target::LLVM_InitializeNativeTarget();
            }
            if code != 0 {
                panic!("unable to initialize native target");
            }
        });

        let llref;
        unsafe {
            let target_triple = LLVMGetDefaultTargetTriple();
            let mut target = ptr::null_mut();
            let mut msg_ptr = ptr::null_mut();
            let code = LLVMGetTargetFromTriple(
                target_triple,
                &mut target as *mut LLVMTargetRef,
                &mut msg_ptr as *mut *mut c_char,
            );
            if code != 0 {
                return Err(Error::from_ptr(msg_ptr));
            }
            llref = LLVMCreateTargetMachine(
                target,
                target_triple,
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
    pub fn builder() -> TargetMachineBuilder {
        TargetMachineBuilder::new()
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
