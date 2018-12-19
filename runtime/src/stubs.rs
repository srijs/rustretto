use std::ptr;

use libc::c_void;

use super::Ref;

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object4initIu9Jc7c6d146EEvv(_this: Ref) {}

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
pub unsafe extern "C" fn _ZN4java4lang6Object6equalsIu9J70818185EEu7booleanN4java4lang6ObjectE(
    _this: Ref,
    _other: Ref,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object5cloneIu9J117cf78dEEN4java4lang6ObjectEv(
    _this: Ref,
) -> Ref {
    Ref::null()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8toStringIu9J7da86304EEN4java4lang6StringEv(
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
pub unsafe extern "C" fn _ZN4java4lang6Object4waitIu9J70446489EEvli(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object4waitIu9J70446489EEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8finalizeIu9J4558e90cEEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6System3outv03getE(_this: Ref) -> Ref {
    Ref {
        object: ptr::null(),
        vtable: crate::io::VTABLE_PRINTSTREAM as *const crate::io::VTablePrintStream
            as *const c_void,
    }
}
