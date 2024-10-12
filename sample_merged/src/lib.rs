#[inline(never)]
#[cfg(target_arch = "x86_64")]
pub fn merged_0() {
    let simd_reg = unsafe {
        std::arch::x86_64::_mm_set_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0)
    };
    std::hint::black_box(simd_reg);
}

#[inline(never)]
#[cfg(target_arch = "x86_64")]
pub fn merged_1() {
    let simd_reg = unsafe {
        std::arch::x86_64::_mm_set_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0)
    };
    std::hint::black_box(simd_reg);
}

#[inline(never)]
pub extern "C" fn extern_c_0() -> u32 {
    2
}

#[inline(never)]
pub extern "C" fn extern_c_1() -> u32 {
    1 + 1
}

#[inline(never)]
pub fn plain_0() -> u32 {
    1
}

#[inline(never)]
pub fn plain_1() -> u32 {
    2 - 1
}
