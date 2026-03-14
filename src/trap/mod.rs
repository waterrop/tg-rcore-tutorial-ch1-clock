
use riscv::register::{
    stvec, scause,
    sie, sstatus
};
use tg_sbi::{console_putchar, rdtime, set_timer};
use core::arch::global_asm;
global_asm!(include_str!("trap.S"));

pub fn init() {
    // 声明外部汇编入口
    unsafe extern "C" {
        fn __alltraps();
    }

    unsafe {
        // 设置 stvec 指向中断入口（你之前修复好的正确写法）
        stvec::write(
            __alltraps as unsafe extern "C" fn() as usize,
            stvec::TrapMode::Direct
        );

        // 开启 S 态时钟中断使能
        sie::set_stimer();
        // 开启全局中断
        sstatus::set_sie();
    }
}

/// Rust 高级中断分发函数
#[unsafe(no_mangle)]
pub fn trap_handler() {
    let scause = scause::read();

    if scause.is_interrupt() {
        match scause.code() {
            5 => {
                // 时钟中断
                console_putchar(b't');
                let interval = 10_000_000u64;   // 定时器间隔：10,000,000 时钟周期 ≈ 1 秒 (假设 10MHz 时钟)
                let current_time = rdtime();
                set_timer(current_time + interval);
            }
            _ => {
                for c in b"[trap] unknown interrupt\n" {   // 打印信息
                    console_putchar(*c);
                }
                loop {}
            }
        }
    } else {
        for c in b"[trap] no exception\n" {   // 打印信息
            console_putchar(*c);
        }
        loop {}
    }
    /*
    // 返回用户态/内核态继续执行
    unsafe {
        asm!("sret", options(noreturn));
    }
    */
}
