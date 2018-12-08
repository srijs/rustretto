use std::ffi::CString;
use std::path::Path;
use std::ptr;

use libc::c_char;
use llvm_sys::target_machine::*;

use error::Error;
use message::Message;
use module::Module;

pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}

pub struct TargetMachineBuilder {
    triple: *const ::libc::c_char,
    cpu: Message,
    features: Message,
    level: LLVMCodeGenOptLevel,
    reloc: LLVMRelocMode,
    code_model: LLVMCodeModel,
}

impl TargetMachineBuilder {
    pub fn host() -> Self {
        let triple;
        let cpu;
        let features;

        unsafe {
            triple = LLVMGetDefaultTargetTriple();
            cpu = Message {
                inner: LLVMGetHostCPUName(),
            };
            features = Message {
                inner: LLVMGetHostCPUFeatures(),
            };
        }

        let level = LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault;
        let reloc = LLVMRelocMode::LLVMRelocDefault;
        let code_model = LLVMCodeModel::LLVMCodeModelDefault;

        TargetMachineBuilder {
            triple,
            cpu,
            features,
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

    pub fn build(self) -> TargetMachine {
        let llref;
        unsafe {
            let mut target = ptr::null_mut();
            let mut msg_ptr = ptr::null_mut();
            LLVMGetTargetFromTriple(
                self.triple,
                &mut target as *mut LLVMTargetRef,
                &mut msg_ptr as *mut *mut c_char,
            );
            llref = LLVMCreateTargetMachine(
                target,
                self.triple,
                self.cpu.inner,
                self.features.inner,
                self.level,
                self.reloc,
                self.code_model,
            );
        }
        TargetMachine { llref }
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
    pub fn emit_to_file(
        &self,
        module: &Module,
        file_type: FileType,
        path: &Path,
    ) -> Result<(), Error> {
        let cpath = CString::new(path.to_str().unwrap()).unwrap();
        let cgen_ftype = match file_type {
            FileType::Assembly => LLVMCodeGenFileType::LLVMAssemblyFile,
            FileType::Object => LLVMCodeGenFileType::LLVMObjectFile,
        };
        unsafe {
            let mut msg_ptr = ptr::null_mut();
            LLVMTargetMachineEmitToFile(
                self.llref,
                module.llref,
                cpath.as_ptr() as *mut c_char,
                cgen_ftype,
                &mut msg_ptr as *mut *mut c_char,
            );
        }
        Ok(())
    }
}

impl Drop for TargetMachine {
    fn drop(&mut self) {
        unsafe { LLVMDisposeTargetMachine(self.llref) }
    }
}
