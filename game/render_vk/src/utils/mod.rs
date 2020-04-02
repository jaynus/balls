pub mod reflect;

/// Cast a slice from one POD type to another.
pub fn cast_slice<A, B>(slice: &[A]) -> &[B] {
    use std::slice;

    let raw_len = std::mem::size_of::<A>().wrapping_mul(slice.len());
    let len = raw_len / std::mem::size_of::<B>();
    assert_eq!(raw_len, std::mem::size_of::<B>().wrapping_mul(len));
    unsafe { slice::from_raw_parts(slice.as_ptr() as *const B, len) }
}

/// Cast a slice from one POD type to another.
pub fn cast_slice_mut<A, B>(slice: &mut [A]) -> &mut [B] {
    use std::slice;

    let raw_len = std::mem::size_of::<A>().wrapping_mul(slice.len());
    let len = raw_len / std::mem::size_of::<B>();
    assert_eq!(raw_len, std::mem::size_of::<B>().wrapping_mul(len));
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut B, len) }
}

#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = ::std::mem::zeroed();
            (&b.$field as *const _ as isize) - (&b as *const _ as isize)
        }
    }};
}
