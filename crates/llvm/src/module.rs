use std::ffi::CStr;
use std::ptr;

use libc::c_char;
use llvm_sys::bit_writer::*;
use llvm_sys::core::*;
use llvm_sys::ir_reader::*;
use llvm_sys::linker::*;
use llvm_sys::prelude::*;

use crate::buffer::MemoryBuffer;
use crate::error::Error;

pub struct Module {
    pub(crate) llref: LLVMModuleRef,
}

impl Module {
    pub fn new(name: &str) -> Self {
        let llref;

        unsafe {
            llref = LLVMModuleCreateWithName(ptr::null());
            LLVMSetModuleIdentifier(llref, name.as_ptr() as *const c_char, name.len());
            LLVMSetSourceFileName(llref, name.as_ptr() as *const c_char, name.len());
        };

        Module { llref }
    }

    pub fn parse_ir(bytes: &[u8]) -> Result<Self, Error> {
        unsafe {
            let ctx = LLVMGetGlobalContext();
            let buf = LLVMCreateMemoryBufferWithMemoryRangeCopy(
                bytes.as_ptr() as *const i8,
                bytes.len(),
                ptr::null(),
            );
            let mut llref = LLVMModuleCreateWithName(ptr::null());
            let mut msg_ptr = ptr::null_mut();
            let code = LLVMParseIRInContext(
                ctx,
                buf,
                &mut llref as *mut LLVMModuleRef,
                &mut msg_ptr as *mut *mut c_char,
            );
            if code == 0 {
                Ok(Module { llref })
            } else {
                let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
                Err(Error { message })
            }
        }
    }

    pub fn set_source_file_name(&mut self, source_file_name: &str) {
        unsafe {
            LLVMSetSourceFileName(
                self.llref,
                source_file_name.as_ptr() as *const c_char,
                source_file_name.len(),
            )
        }
    }

    pub fn link(&mut self, other: Self) -> Result<(), Error> {
        unsafe {
            let code = LLVMLinkModules2(self.llref, other.llref);
            std::mem::forget(other); // ensure we're not calling the deconstructor
            if code == 0 {
                Ok(())
            } else {
                Err(Error {
                    message: "linker error".into(),
                })
            }
        }
    }

    pub fn to_bitcode(&self) -> MemoryBuffer {
        let llref;
        unsafe {
            llref = LLVMWriteBitcodeToMemoryBuffer(self.llref);
        }
        MemoryBuffer { llref }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe { LLVMDisposeModule(self.llref) }
    }
}
