#[no_mangle]
pub extern "C" fn add(left: usize, right: usize) -> usize {
    left + right
}

#[no_mangle]
pub extern "C" fn sub(left: usize, right: usize) -> usize {
    left - right
}

#[no_mangle]
pub extern "C" fn _mul(left: usize, right: usize) -> usize {
    left * right
}
