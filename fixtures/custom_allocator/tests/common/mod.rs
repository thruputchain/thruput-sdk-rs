#![allow(unused)]

/// What `CustomAllocator::churn` returns.
pub fn churn_sum(rounds: u32, bytes: u32) -> u32 {
    let mut sum = 0u32;
    for round in 0..rounds {
        for i in 0..bytes {
            sum = sum.wrapping_add(i.wrapping_add(round) as u8 as u32);
        }
    }
    sum
}

/// What `CustomAllocator::grow` returns.
pub fn grow_sum(len: u32) -> u32 {
    (0..len).fold(0u32, |s, i| s.wrapping_add(i as u8 as u32))
}
