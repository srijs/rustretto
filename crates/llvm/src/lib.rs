extern crate libc;
extern crate llvm_sys;

mod buffer;
mod error;
mod message;
mod module;

pub use buffer::MemoryBuffer;
pub use error::Error;
pub use module::Module;
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
