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
pub fn two_num() -> u32 {
    2
}

#[inline(never)]
pub fn one_num() -> u32 {
    1
}

#[inline(never)]
pub fn one_plus_one() -> u32 {
    1 + 1
}

#[inline(never)]
pub fn two_minus_one() -> u32 {
    2 - 1
}
