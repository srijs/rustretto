use std::fmt;
use std::ptr;
use std::sync::Once;

use libc::{c_char, c_uint};
use llvm_sys::prelude::*;
use llvm_sys::target::*;
use llvm_sys::target_machine::*;

use crate::buffer::MemoryBuffer;
use crate::error::Error;
use crate::message::Message;
use crate::module::Module;

pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}

static INIT_NATIVE_TARGET: Once = Once::new();
static INIT_NATIVE_ASM_PRINTER: Once = Once::new();

fn init_native_target() {
    INIT_NATIVE_TARGET.call_once(|| {
        let code;
        unsafe {
            code = llvm_sys::target::LLVM_InitializeNativeTarget();
        }
        if code != 0 {
            panic!("unable to initialize native target");
        }
    });
}

fn init_native_asm_printer() {
    INIT_NATIVE_ASM_PRINTER.call_once(|| {
        let code;
        unsafe {
            code = llvm_sys::target::LLVM_InitializeNativeAsmPrinter();
        }
        if code != 0 {
            panic!("unable to initialize native asm printer");
        }
    });
}

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

    pub fn set_opt_level(&mut self, level: OptLevel) {
        self.level = match level {
            OptLevel::None => LLVMCodeGenOptLevel::LLVMCodeGenLevelNone,
            OptLevel::Less => LLVMCodeGenOptLevel::LLVMCodeGenLevelLess,
            OptLevel::Default => LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
            OptLevel::Aggressive => LLVMCodeGenOptLevel::LLVMCodeGenLevelAggressive,
        }
    }

    pub fn build(self) -> Result<TargetMachine, Error> {
        init_native_target();
        init_native_asm_printer();

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

    pub fn triple(&self) -> TargetTriple {
        let ptr;
        unsafe {
            ptr = LLVMGetTargetMachineTriple(self.llref);
        }
        TargetTriple {
            inner: Message { inner: ptr },
        }
    }

    pub fn data_layout(&self) -> TargetDataLayout {
        let llref;
        unsafe {
            llref = LLVMCreateTargetDataLayout(self.llref);
        }
        TargetDataLayout { llref }
    }

    pub fn emit_to_buffer(&self, module: &Module, typ: FileType) -> Result<MemoryBuffer, Error> {
        let mut llref = ptr::null_mut();
        let codegen = match typ {
            FileType::Assembly => LLVMCodeGenFileType::LLVMAssemblyFile,
            FileType::Object => LLVMCodeGenFileType::LLVMObjectFile,
        };
        let mut err_msg = ptr::null_mut();
        unsafe {
            let code = LLVMTargetMachineEmitToMemoryBuffer(
                self.llref,
                module.llref,
                codegen,
                &mut err_msg as *mut *mut c_char,
                &mut llref as *mut LLVMMemoryBufferRef,
            );
            if code != 0 {
                return Err(Error::from_ptr(err_msg));
            }
        }
        Ok(MemoryBuffer { llref })
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

pub struct TargetTriple {
    inner: Message,
}

impl TargetTriple {
    pub fn get_macosx_version(&self) -> (u32, u32, u32) {
        let mut major = 0 as c_uint;
        let mut minor = 0 as c_uint;
        let mut micro = 0 as c_uint;
        unsafe {
            LLVMTripleGetMacOSXVersion(
                self.inner.inner,
                &mut major as *mut c_uint,
                &mut minor as *mut c_uint,
                &mut micro as *mut c_uint,
            );
        }
        (major as u32, minor as u32, micro as u32)
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

extern "C" {
    fn LLVMTripleGetMacOSXVersion(
        triple: *const c_char,
        major: *mut c_uint,
        minor: *mut c_uint,
        micro: *mut c_uint,
    );
}
