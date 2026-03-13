//! S-Mode Interrupt Enable (sie) 寄存器操作
//!
//! 本模块提供 S-mode 定时器中断的启用功能。

/// 启用 S-mode 定时器中断
///
/// 需要完成以下两步：
/// 1. 设置 sstatus 寄存器的 SIE 位 (bit 1)
/// 2. 设置 sie 寄存器的 STIE 位 (bit 5)
#[inline]
pub fn set_stimer(){
    unsafe{
        // 1.启动全局中断
        let mut sstatus: usize;     // 读取sstatus寄存器
        core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus);
        sstatus |= 1 << 1;          // 设置SIE位
        core::arch::asm!("csrw sstatus, {0}", in(reg) sstatus);  // 写回sstatus寄存器
        // 2.读取sie寄存器
        let mut sie: usize;
        core::arch::asm!("csrr {0}, sie", out(reg) sie);
        sie |= 1 << 5;
        core::arch::asm!("csrw sie, {0}", in(reg) sie);
        /*
        // 3.设置mideleg，委托时钟中断给S-mode处理
        let mut mideleg: usize;
        core::arch::asm!("csrr {0}, mideleg", out(reg) mideleg);
        mideleg |= 1 << 5;  // 设置 STIP 位
        core::arch::asm!("csrw mideleg, {0}", in(reg) mideleg);
        */
    }
}