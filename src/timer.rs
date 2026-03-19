pub const CLOCK_FREQ: u64 = 12_500_000;

#[inline]
pub fn read_time() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) ticks);
    }
    ticks
}

#[inline]
#[allow(dead_code)]
pub fn get_time_ms() -> u64 {
    read_time() * 1000 / CLOCK_FREQ
}

#[inline]
#[allow(dead_code)]
pub fn ticks_per_sec() -> u64 {
    CLOCK_FREQ
}
