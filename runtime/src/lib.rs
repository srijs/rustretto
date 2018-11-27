extern crate libc;

use std::ptr;

use libc::{c_char, c_void};

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
pub unsafe extern "C" fn _Jm_java_lang_Object__init__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_registerNatives__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_getClass__Ljava_lang_Class_2(_this: Ref) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_hashCode__I(_this: Ref) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_equals__Z__Ljava_lang_Object_2(
    _this: Ref,
    _other: Ref,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_clone__Ljava_lang_Object_2(_this: Ref) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_toString__Ljava_lang_String_2(_this: Ref) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_notify__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_notifyAll__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_wait__Z__J(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_wait__Z__JI(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_wait__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object_finalize__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object__clinit__Z(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jf_java_lang_System_out__get(_this: Ref) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_io_PrintStream_println__Z__Ljava_lang_String_2(
    _this: Ref,
    string: Ref,
) {
    libc::puts(string.object as *const c_char);
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_ldstr(_len: i32, bytes: *const i8) -> Ref {
    // horrible hack! :(
    Ref {
        object: bytes as *const c_void,
        vtable: ptr::null(),
    }
}
