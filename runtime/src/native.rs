use crate::Ref;

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object15registerNativesIu9Jed9fc4b9EEvv() {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8getClassIu9Jd57352f6EEN4java4lang5ClassEv(
    _this: Ref,
) -> Ref {
    unimplemented!();
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object8hashCodeIu9J7c7c3589EEiv(this: Ref) -> i32 {
    this.identity_hash_code()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object5cloneIu9J117cf78dEEN4java4lang6ObjectEv(
    _this: Ref,
) -> Ref {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object6notifyIu9Jec9f6595EEvv(_this: Ref) {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object9notifyAllIu9J01f1085cEEvv(_this: Ref) {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang6Object4waitIu9J70446489EEvl(_this: Ref) {
    unimplemented!()
}
