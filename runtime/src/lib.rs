use std::ptr;

use backtrace::Backtrace;
use libc::c_void;

mod io;
pub mod native;
pub mod stubs;

#[repr(C)]
pub struct Ref {
    object: *const c_void,
    vtable: *const c_void,
}

unsafe impl Sync for Ref {}

impl Ref {
    pub fn null() -> Self {
        Ref {
            object: ptr::null(),
            vtable: ptr::null(),
        }
    }

    pub fn identity_hash_code(&self) -> i32 {
        self.object as i32
    }
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_throw(_throwable: Ref) {
    let backtrace = Backtrace::new();
    println!("Exception {:?}", backtrace);
    std::process::exit(1);
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_ldstr(_len: i32, bytes: *const i8) -> Ref {
    // horrible hack! :(
    Ref {
        object: bytes as *const c_void,
        vtable: ptr::null(),
    }
}
