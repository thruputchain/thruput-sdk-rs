//! `__atomic_*` libcalls. The zkvm-PAL target lowers atomics to these calls
//! (no A extension). Groundhog contracts are single-threaded, so plain memory
//! operations are exact.
#![allow(clippy::missing_safety_doc)]

macro_rules! atomics {
    ($t:ty, $n:literal) => {
        paste_atomics!($t, $n);
    };
}

macro_rules! paste_atomics {
    ($t:ty, $n:literal) => {
        const _: () = {
            #[unsafe(export_name = concat!("__atomic_load_", $n))]
            unsafe extern "C" fn load(p: *const $t, _o: i32) -> $t { unsafe { p.read_volatile() } }
            #[unsafe(export_name = concat!("__atomic_store_", $n))]
            unsafe extern "C" fn store(p: *mut $t, v: $t, _o: i32) { unsafe { p.write_volatile(v) } }
            #[unsafe(export_name = concat!("__atomic_exchange_", $n))]
            unsafe extern "C" fn exchange(p: *mut $t, v: $t, _o: i32) -> $t {
                unsafe { let old = p.read_volatile(); p.write_volatile(v); old }
            }
            #[unsafe(export_name = concat!("__atomic_compare_exchange_", $n))]
            unsafe extern "C" fn cmpxchg(p: *mut $t, expected: *mut $t, desired: $t, _w: bool, _s: i32, _f: i32) -> bool {
                unsafe {
                    let cur = p.read_volatile();
                    if cur == *expected { p.write_volatile(desired); true } else { *expected = cur; false }
                }
            }
            #[unsafe(export_name = concat!("__atomic_fetch_add_", $n))]
            unsafe extern "C" fn fadd(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(o.wrapping_add(v)); o } }
            #[unsafe(export_name = concat!("__atomic_fetch_sub_", $n))]
            unsafe extern "C" fn fsub(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(o.wrapping_sub(v)); o } }
            #[unsafe(export_name = concat!("__atomic_fetch_and_", $n))]
            unsafe extern "C" fn fand(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(o & v); o } }
            #[unsafe(export_name = concat!("__atomic_fetch_or_", $n))]
            unsafe extern "C" fn f_or(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(o | v); o } }
            #[unsafe(export_name = concat!("__atomic_fetch_xor_", $n))]
            unsafe extern "C" fn fxor(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(o ^ v); o } }
            #[unsafe(export_name = concat!("__atomic_fetch_nand_", $n))]
            unsafe extern "C" fn fnand(p: *mut $t, v: $t, _o: i32) -> $t { unsafe { let o = p.read_volatile(); p.write_volatile(!(o & v)); o } }
        };
    };
}

atomics!(u8, "1");
atomics!(u16, "2");
atomics!(u32, "4");
atomics!(u64, "8");
