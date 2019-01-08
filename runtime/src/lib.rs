use std::ptr;

use backtrace::Backtrace;
use libc::c_void;

mod io;
pub mod native;
pub mod stubs;

extern "C" {
    #[no_mangle]
    pub static _ZTVN4java4lang6ObjectE: *const c_void;
}

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
pub unsafe extern "C" fn _Jrt_new(size: u64, vtable: *const i8) -> Ref {
    let object = libc::malloc(size as usize);
    Ref {
        object: object,
        vtable: vtable as *const c_void,
    }
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_new_array(count: u32, component_size: u64) -> Ref {
    let size = 4 + count as usize * component_size as usize;
    let object = libc::malloc(size);
    ptr::write(object as *mut u32, count);
    Ref {
        object,
        vtable: _ZTVN4java4lang6ObjectE,
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
