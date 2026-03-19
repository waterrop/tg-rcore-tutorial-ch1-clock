use crate::timer;
use tg_sbi::{console_putchar, set_timer, shutdown};

const HELLO_WORLD: &[u8] = b"hello world";

static mut CHAR_IDX: usize = 0;
static mut START_TIME_MS: u64 = 0;

pub fn init(start_ms: u64) {
    unsafe {
        START_TIME_MS = start_ms;
        CHAR_IDX = 0;

        core::arch::asm!(
            "la   t0, {entry}",
            "csrw stvec, t0",
            entry = sym s_trap_entry,
        );

        core::arch::asm!("li t0, 0x20", "csrs sie, t0");
        core::arch::asm!("csrsi sstatus, 0x2");
    }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn s_trap_entry() {
    core::arch::naked_asm!(
        "addi sp, sp, -256",
        "sd   x0,   0(sp)",
        "sd   x1,   8(sp)",
        "sd   x2,  16(sp)",
        "sd   x3,  24(sp)",
        "sd   x4,  32(sp)",
        "sd   x5,  40(sp)",
        "sd   x6,  48(sp)",
        "sd   x7,  56(sp)",
        "sd   x8,  64(sp)",
        "sd   x9,  72(sp)",
        "sd   x10, 80(sp)",
        "sd   x11, 88(sp)",
        "sd   x12, 96(sp)",
        "sd   x13,104(sp)",
        "sd   x14,112(sp)",
        "sd   x15,120(sp)",
        "sd   x16,128(sp)",
        "sd   x17,136(sp)",
        "sd   x18,144(sp)",
        "sd   x19,152(sp)",
        "sd   x20,160(sp)",
        "sd   x21,168(sp)",
        "sd   x22,176(sp)",
        "sd   x23,184(sp)",
        "sd   x24,192(sp)",
        "sd   x25,200(sp)",
        "sd   x26,208(sp)",
        "sd   x27,216(sp)",
        "sd   x28,224(sp)",
        "sd   x29,232(sp)",
        "sd   x30,240(sp)",
        "sd   x31,248(sp)",
        "csrr a0, scause",
        "call {handler}",
        "ld   x1,   8(sp)",
        "ld   x3,  24(sp)",
        "ld   x4,  32(sp)",
        "ld   x5,  40(sp)",
        "ld   x6,  48(sp)",
        "ld   x7,  56(sp)",
        "ld   x8,  64(sp)",
        "ld   x9,  72(sp)",
        "ld   x10, 80(sp)",
        "ld   x11, 88(sp)",
        "ld   x12, 96(sp)",
        "ld   x13,104(sp)",
        "ld   x14,112(sp)",
        "ld   x15,120(sp)",
        "ld   x16,128(sp)",
        "ld   x17,136(sp)",
        "ld   x18,144(sp)",
        "ld   x19,152(sp)",
        "ld   x20,160(sp)",
        "ld   x21,168(sp)",
        "ld   x22,176(sp)",
        "ld   x23,184(sp)",
        "ld   x24,192(sp)",
        "ld   x25,200(sp)",
        "ld   x26,208(sp)",
        "ld   x27,216(sp)",
        "ld   x28,224(sp)",
        "ld   x29,232(sp)",
        "ld   x30,240(sp)",
        "ld   x31,248(sp)",
        "addi sp, sp, 256",
        "sret",
        handler = sym trap_handler,
    );
}

extern "C" fn trap_handler(scause: usize) {
    let is_interrupt = (scause as isize) < 0;
    let exception_code = scause & !(1usize << 63);
    if is_interrupt && exception_code == 5 {
        let next = timer::read_time() + timer::CLOCK_FREQ;
        set_timer(next);
        unsafe {
            let idx = CHAR_IDX;
            if idx < HELLO_WORLD.len() {
                console_putchar(HELLO_WORLD[idx]);
                console_putchar(b'\n');
                CHAR_IDX = idx + 1;
                if CHAR_IDX == HELLO_WORLD.len() {
                    let elapsed_ms = timer::get_time_ms() - START_TIME_MS;
                    let elapsed_s = elapsed_ms / 1000;
                    let remaining_ms = elapsed_ms % 1000;
                    print_str(b"elapsed: ");
                    print_u64(elapsed_s);
                    print_str(b" s ");
                    print_u64(remaining_ms);
                    print_str(b" ms\n");
                    shutdown(false);
                }
            }
        }
    }
}

fn print_str(s: &[u8]) {
    for &c in s {
        console_putchar(c);
    }
}

fn print_u64(mut n: u64) {
    if n == 0 {
        console_putchar(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    while n > 0 {
        buf[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in (0..len).rev() {
        console_putchar(buf[i]);
    }
}
