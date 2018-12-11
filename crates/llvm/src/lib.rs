extern crate libc;
extern crate llvm_sys;

mod buffer;
mod error;
mod message;
mod module;

pub use crate::buffer::MemoryBuffer;
pub use crate::error::Error;
pub use crate::module::Module;
pub mod codegen;
pub mod transform;

pub fn init() {
    unsafe {
        llvm_sys::target::LLVM_InitializeAllTargetInfos();
        llvm_sys::target::LLVM_InitializeAllTargets();
        llvm_sys::target::LLVM_InitializeAllTargetMCs();
        llvm_sys::target::LLVM_InitializeAllAsmPrinters();
    }
}
