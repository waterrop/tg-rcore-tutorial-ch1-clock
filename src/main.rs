//! # 第一章：应用程序与基本执行环境
//!
//! 本章实现了一个最简单的 RISC-V S 态裸机程序，展示操作系统的最小执行环境。
//!
//! ## 关键概念
//!
//! - `#![no_std]`：不使用 Rust 标准库，改用不依赖操作系统的核心库 `core`
//! - `#![no_main]`：不使用标准的 `main` 入口，自定义裸函数 `_start` 作为入口
//! - 裸函数（naked function）：不生成函数序言/尾声，可在无栈环境下执行
//! - SBI（Supervisor Binary Interface）：S 态软件向 M 态固件请求服务的标准接口
//!
//! 教程阅读建议：
//!
//! - 先看 `_start`：理解无运行时情况下的最小启动流程；
//! - 再看 `rust_main`：理解最小 I/O 路径（SBI 输出 + 关机）；
//! - 最后看 `panic_handler`：理解 no_std 程序的异常收口方式。

// 不使用标准库，因为裸机环境没有操作系统提供系统调用支持
#![no_std]
// 不使用标准入口，因为裸机环境没有 C runtime 进行初始化
#![no_main]
// RISC-V64 架构下启用严格警告和文档检查
#![cfg_attr(target_arch = "riscv64", deny(warnings, missing_docs))]
// 非 RISC-V64 架构允许死代码（用于 cargo publish --dry-run 在主机上通过编译）
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]

// 引入 SBI 调用库，提供 console_putchar（输出字符）和 shutdown（关机）功能
// 启用 nobios 特性后，tg_sbi 内建了 M-mode 启动代码，无需外部 SBI 固件
use tg_sbi::{console_putchar, shutdown, set_timer};

// 引入sie模块
mod sie;

/// 读取当前时间 (mtime 寄存器)
///
/// RISC-V 中有一个内存映射的定时器 (mtime)，可以通过 rdtime 指令读取。
/// 返回值是以时钟 tick 为单位的当前时间。
///
/// QEMU virt 中，时钟频率通常是 10MHz (每微秒 10 个 tick)
#[inline]
fn rdtime() -> u64{
    let mut time: u64;
    unsafe{ core::arch::asm!("rdtime {}", out(reg) time); }
    time
}


/// S 态程序入口点。
///
/// 这是一个裸函数（naked function），放置在 `.text.entry` 段，
/// 链接脚本将其安排在地址 `0x80200000`。
///
/// 裸函数不生成函数序言和尾声，因此可以在没有栈的情况下执行。
/// 它完成两件事：
/// 1. 设置栈指针 `sp`，指向栈顶（栈从高地址向低地址增长）
/// 2. 跳转到 Rust 主函数 `rust_main`
#[cfg(target_arch = "riscv64")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start() -> ! {
    // 栈大小：4 KiB
    const STACK_SIZE: usize = 4096;

    // 在 .bss.uninit 段中分配栈空间
    #[unsafe(link_section = ".bss.uninit")]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::naked_asm!(
        "la sp, {stack} + {stack_size}",    // 将 sp 设置为栈顶地址
        "j  {main}",                        // 跳转到 rust_main
        stack_size = const STACK_SIZE,
        stack      =   sym STACK,
        main       =   sym rust_main,
    )
}

/// S 态主函数：定时输出字符
///
/// 工作流程：
/// 1. 启用定时器中断
/// 2. 设置第一次定时器中断
/// 3. 打印启动消息
/// 4. 进入无限循环，等待定时器中断触发
/// 通过 SBI 的 `console_putchar` 逐字节输出字符串，
/// 然后调用 `shutdown` 正常关机退出 QEMU。
extern "C" fn rust_main() -> ! {
    // 开启S特权级时钟中断
    sie::set_stimer();
    /*
    // 调试：检查 sstatus
    let sstatus: usize;
    unsafe { core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus) }
    console_putchar(if (sstatus & 0x2) != 0 { b'Y' } else { b'N' });  // Y = OK, N = SIE not set

    // 调试：检查 sie
    let sie: usize;
    unsafe { core::arch::asm!("csrr {0}, sie", out(reg) sie) }
    console_putchar(if (sie & 0x20) != 0 { b'Y' } else { b'N' });  // Y = OK, N = STIE not set

    // 调试：检查 mideleg（中断委托）
    let mideleg: usize;
    unsafe { core::arch::asm!("csrr {0}, mideleg", out(reg) mideleg) }
    console_putchar(if (mideleg & 0x20) != 0 { b'Y' } else { b'N' });  // Y = OK, N = STIP not delegated
    */
    // 设置第一次定时器中断
    let interval = 10_000_000u64;   // 定时器间隔：10,000,000 时钟周期 ≈ 1 秒 (假设 10MHz 时钟)
    let current_time = rdtime();
    let mut next_tick = rdtime() + interval;
    let mut cnt: usize = 0;
    set_timer(current_time + interval);
    for c in b"Time started!\n" {   // 打印启动信息
        console_putchar(*c);
    }
    // 无限循环等待中断
    loop{
        let current = rdtime();
        if current >= next_tick {
            if cnt == 10 {
                for c in b"\nShutdown!\n" {   // 打印关机信息
                    console_putchar(*c);
                }
                break;
            }
            console_putchar(b't');
            next_tick = current + interval;
            set_timer(next_tick);
            cnt += 1;
        }
        // 1.读取scause，判断中断类型
        let mut scause: usize;
        unsafe{ core::arch::asm!("csrr {0}, scause", out(reg) scause); }
        // scause 最高位为 1 表示中断，为 0 表示异常
        // 最低几位表示中断类型：
        //   - 5 (0b00101): S-mode 定时器中断 (STIP)
        //   - 1 (0b00001): S-mode 软件中断 (SSIP)
        //   - 9 (0b01001): S-mode 外部中断 (SEIP)
        if scause == 0x8000000000000005{
            if cnt == 10 {
                for c in b"Shutdown!\n" {   // 打印关机信息
                    console_putchar(*c);
                }
                break;
            }
            // 输出字符
            console_putchar(b't');
            // 重新设置下一次定时器中断
            let interval = 10_000_000u64;
            let current_time = rdtime();
            set_timer(current_time + interval);
            cnt += 1;
        }
        // 清除挂起的定时器中断
        unsafe{ core::arch::asm!("csrc sip, {0}", in(reg) (1 << 5)); }
    }

    shutdown(false) // false 表示正常关机
}

/*
/// S-mode trap handler (中断/异常处理函数)
///
/// 当发生定时器中断时，CPU 会自动跳转到 stvec 指向的地址。
/// 我们需要在 trap handler 中：
/// 1. 判断中断类型
/// 2. 处理定时器中断（输出字符 + 重新设置定时器）
/// 3. 返回原程序继续执行
#[unsafe(no_mangle)]
extern "C" fn s_trap_handler(){
    //console_putchar(b'X');
    // 保存寄存器（在栈上分配空间）
    unsafe{
        core::arch::asm!(
            "addi sp, sp, -64",  // 分配栈空间
            "sd ra, 0(sp)",      // 保存 ra
            "sd t0, 8(sp)",      // 保存 t0
            "sd t1, 16(sp)",     // 保存 t1
            "sd a0, 24(sp)",     // 保存 a0
        );
    }
    // 1.读取scause，判断中断类型
    let mut scause: usize;
    unsafe{ core::arch::asm!("csrr {0}, scause", out(reg) scause); }
    // scause 最高位为 1 表示中断，为 0 表示异常
    // 最低几位表示中断类型：
    //   - 5 (0b00101): S-mode 定时器中断 (STIP)
    //   - 1 (0b00001): S-mode 软件中断 (SSIP)
    //   - 9 (0b01001): S-mode 外部中断 (SEIP)
    let is_interrupt = (scause >> 63) != 0;
    let interrupt_code = scause & 0xFFFF_FFFF;
    if is_interrupt && interrupt_code == 5{
        // 输出字符
        console_putchar(b't');
        // 重新设置下一次定时器中断
        let interval = 10_000_000u64;
        let current_time = rdtime();
        set_timer(current_time + interval);
    }
    // 清除挂起的定时器中断
    unsafe{ core::arch::asm!("csrc sip, {0}", in(reg) (1 << 5)); }
    // 恢复寄存器
    unsafe{
        core::arch::asm!(
            "ld ra, 0(sp)",
            "ld t0, 8(sp)",
            "ld t1, 16(sp)",
            "ld a0, 24(sp)",
            "addi sp, sp, 64",   // 释放栈空间
            "sret",              // 返回到中断发生的位置
        );
    }
    
}
*/

/// panic 处理函数。
///
/// `#![no_std]` 环境下必须自行实现。发生 panic 时以异常状态关机。
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    shutdown(true) // true 表示异常关机
}

/// 非 RISC-V64 架构的占位模块。
///
/// 提供 `main` 等符号，使得在主机平台（如 x86_64）上也能通过编译，
/// 满足 `cargo publish --dry-run` 和 `cargo test` 的需求。
#[cfg(not(target_arch = "riscv64"))]
mod stub {
    /// 主机平台占位入口
    #[unsafe(no_mangle)]
    pub extern "C" fn main() -> i32 {
        0
    }

    /// C 运行时占位
    #[unsafe(no_mangle)]
    pub extern "C" fn __libc_start_main() -> i32 {
        0
    }

    /// Rust 异常处理人格占位
    #[unsafe(no_mangle)]
    pub extern "C" fn rust_eh_personality() {}
}
