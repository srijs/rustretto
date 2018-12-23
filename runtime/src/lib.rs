use std::ptr;

use backtrace::Backtrace;
use libc::c_void;

mod io;
pub mod stubs;

#[repr(C)]
pub struct Ref {
    object: *const c_void,
    vtable: *const c_void,
}

impl Ref {
    pub fn null() -> Self {
        Ref {
            object: ptr::null(),
            vtable: ptr::null(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_throw(_throwable: Ref) -> Ref {
    println!("Exception: {:?}", Backtrace::new());
    std::process::abort();
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_ldstr(_len: i32, bytes: *const i8) -> Ref {
    // horrible hack! :(
    Ref {
        object: bytes as *const c_void,
        vtable: ptr::null(),
    }
}
