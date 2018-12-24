use std::ptr;

use libc::c_void;

use super::Ref;

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8getClassIu9Jd57352f6EEN4java4lang5ClassEv(
    _this: Ref,
) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8hashCodeIu9J7c7c3589EEiv(_this: Ref) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object5cloneIu9J117cf78dEEN4java4lang6ObjectEv(
    _this: Ref,
) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object6notifyIu9Jec9f6595EEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object9notifyAllIu9J01f1085cEEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object4waitIu9J70446489EEvl(_this: Ref) {}

#[no_mangle]
pub static _ZN4java4lang6System3outE: Ref = Ref {
    object: ptr::null(),
    vtable: crate::io::VTABLE_PRINTSTREAM as *const crate::io::VTablePrintStream as *const c_void,
};

#[no_mangle]
pub static _ZTVN4java4lang13StringBuilderE: () = ();

#[no_mangle]
pub static _ZTVN4java4lang24IllegalArgumentExceptionE: () = ();

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang13StringBuilder4initIu9J8cc45093EEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang24IllegalArgumentException4initIu9Jffb6fc97EEvN4java4lang6StringE(
    _this: Ref,
    _string: Ref,
) {
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object15registerNativesIu9Jed9fc4b9EEvv() {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang7Integer11toHexStringIu9Jab2e85aaEEN4java4lang6StringEi(
    _value: i64,
) -> Ref {
    Ref::null()
}
