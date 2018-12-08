use llvm_sys::core::*;
use llvm_sys::prelude::*;
use llvm_sys::transforms::pass_manager_builder::*;

use module::Module;

pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
}

pub struct PassManagerBuilder {
    llref: LLVMPassManagerBuilderRef,
}

impl PassManagerBuilder {
    pub fn new() -> Self {
        let llref;
        unsafe {
            llref = LLVMPassManagerBuilderCreate();
        }
        PassManagerBuilder { llref }
    }

    pub fn set_opt_level(&mut self, level: OptLevel) {
        match level {
            OptLevel::O0 => unsafe {
                LLVMPassManagerBuilderSetOptLevel(self.llref, 0);
            },
            OptLevel::O1 => unsafe {
                LLVMPassManagerBuilderSetOptLevel(self.llref, 1);
            },
            OptLevel::O2 => unsafe {
                LLVMPassManagerBuilderSetOptLevel(self.llref, 2);
            },
            OptLevel::O3 => unsafe {
                LLVMPassManagerBuilderSetOptLevel(self.llref, 3);
            },
        }
    }

    pub fn build(self) -> PassManager {
        let llref;
        unsafe {
            llref = LLVMCreatePassManager();
            LLVMPassManagerBuilderPopulateModulePassManager(self.llref, llref);
            LLVMPassManagerBuilderPopulateLTOPassManager(self.llref, llref, 1, 1);
        }
        PassManager { llref }
    }
}

impl Drop for PassManagerBuilder {
    fn drop(&mut self) {
        unsafe {
            LLVMPassManagerBuilderDispose(self.llref);
        }
    }
}

pub struct PassManager {
    llref: LLVMPassManagerRef,
}

impl PassManager {
    pub fn run(self, module: &mut Module) -> bool {
        let code;
        unsafe {
            code = LLVMRunPassManager(self.llref, module.llref);
        }
        if code == 0 {
            false
        } else {
            true
        }
    }
}

impl Drop for PassManager {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposePassManager(self.llref);
        }
    }
}
