#[no_mangle]
pub extern "C" fn example_add(left: usize, right: usize) -> usize {
    left + right
}
