//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Interrupt, Trap},
    sie, stval, stvec,
};
use tg_sbi::{console_putchar, set_timer, rdtime};


global_asm!(include_str!("trap.S"));

/// Initialize trap handling
pub fn init() {
    for c in b"Trap handling started!\n" {   // 打印启动信息
        console_putchar(*c);
    }
    // 声明外部汇编函数
    unsafe extern "C" {
        fn __alltraps();
    }
    // 设置 stvec 指向中断入口
    unsafe {
        // Direct 模式：直接跳转到 __alltraps 地址
        stvec::write(__alltraps as unsafe extern "C" fn() as usize, TrapMode::Direct);
    }
}

/// enable timer interrupt in supervisor mode
pub fn enable_timer_interrupt() {
    for c in b"S-Mode started!\n" {   // 打印启动信息
        console_putchar(*c);
    }
    unsafe {
        // SIE启用
        core::arch::asm!("csrs sstatus, 2");
        sie::set_stimer();
    }
}

/// trap handler
#[unsafe(no_mangle)]
pub fn trap_handler() {
    let scause = scause::read(); // get trap cause
    let stval = stval::read(); // get extra value
    // trace!("into {:?}", scause.cause());
    let scause_val = scause::read().bits();
    /*
    let sstatus_val: usize;
    let sie_val: usize;

    unsafe {
        core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus_val);
        core::arch::asm!("csrr {0}, sie", out(reg) sie_val);
    }
    */
    console_putchar(b'x');

    // 输出 scause 的原始值（调试用）
    console_putchar(b'0' + ((scause_val >> 60) & 0xF) as u8);
    console_putchar(b'0' + ((scause_val >> 56) & 0xF) as u8);

    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            // 输出字符
            console_putchar(b'b');
            // 重新设置下一次定时器中断
            let interval = 10_000_000u64;
            let current_time = rdtime();
            set_timer(current_time + interval);
        }
        _ => {
            console_putchar(b'p');
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
}
