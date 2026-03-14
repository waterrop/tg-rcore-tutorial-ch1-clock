# ch1-clock 时钟中断调试计划 (第二轮)

## 调试现象总结

根据你的调试结果，关键发现如下：

| 步骤 | 现象 | 结论 |
|------|------|------|
| 1 | 没有 '.' 输出 | 循环可能没有正常执行，或 wfi 后程序没有继续 |
| 2 | sstatus/sie 值异常 | 寄存器读取有问题 |
| 3 | scause 一直为 0 | 没有 trap 发生 |
| 4 | 添加 medeleg/mideleg 读取后，输出 'x' 然后退出 | **关键发现** |

---

## 核心问题分析

### 问题：读取 medeleg/mideleg 触发了 trap！

你的代码：
```rust
csrr {0}, medeleg
csrr {0}, mideleg
```

**这行代码本身就是 trap 的来源！**

在 RISC-V 特权架构中：
- `medeleg` 和 `mideleg` 是 **M-mode CSR**
- **S-mode 没有权限读取这些寄存器**
- 执行 `csrr` 读取这些寄存器会触发 **非法指令异常 (Illegal Instruction)** 或 **访问异常**

这就是为什么：
1. 添加这段代码后触发了 trap → 进入 trap_handler → 输出 'x'
2. scause 不是 SupervisorTimer（是异常，不是中断）
3. 所以没有匹配到 `Trap::Interrupt(Interrupt::SupervisorTimer)` 分支
4. 应该走到 `_ =>` 分支，输出 'p' 并 panic

---

## 为什么没有看到 'p' 和 panic 信息？

可能原因：

### 1. sstatus.SIE 未正确恢复

trap 处理流程：
1. 进入 trap 时，硬件自动清除 sstatus.SIE（禁用中断）
2. trap 处理完成
3. sret 返回，恢复 sstatus

问题：trap.S 中保存/恢复 sstatus 的方式可能有问题，导致 SIE 位没有被正确恢复。

### 2. trap 处理过程中再次触发异常

如果 trap handler 执行过程中再次出现问题，可能导致二次 trap。

---

## 正确的调试步骤

### 步骤 1: 移除错误的 medeleg/mideleg 读取代码

先把这部分代码注释掉：
```rust
loop {
    // 移除这段会触发 trap 的代码！
    /*
    let medeleg: usize;
    let mideleg: usize;
    unsafe {
        core::arch::asm!("csrr {0}, medeleg", out(reg) medeleg);
        core::arch::asm!("csrr {0}, mideleg", out(reg) mideleg);
    }
    */
}
```

### 步骤 2: 检查 sstatus 和 sie（在 trap handler 中）

修改 `trap_handler`，输出更多信息：

```rust
pub fn trap_handler() {
    let scause_val = scause::read().bits();
    let sstatus_val: usize;
    let sie_val: usize;

    unsafe {
        core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus_val);
        core::arch::asm!("csrr {0}, sie", out(reg) sie_val);
    }

    console_putchar(b'x');

    // 输出 scause 的原始值（调试用）
    console_putchar(b'0' + ((scause_val >> 60) & 0xF) as u8);
    console_putchar(b'0' + ((scause_val >> 56) & 0xF) as u8);
    // ... 更多输出

    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            console_putchar(b'b');
            let interval = 10_000_000u64;
            let current_time = rdtime();
            set_timer(current_time + interval);
        }
        _ => {
            console_putchar(b'p');
            // 输出 scause 详细信息
            panic!("trap: {:?}", scause.cause());
        }
    }
}
```

### 步骤 3: 验证中断委托是否正确

**不要直接读取 medeleg/mideleg！**

检查 M-mode 是否已经正确委托定时器中断。有两种方法：

#### 方法 A: 通过SBI调用（如果有）

某些 SBI 实现支持查询中断委托状态。

#### 方法 B: 间接验证

如果定时器中断正常工作，则说明委托已正确设置。不需要直接读取这些 CSR。

---

## 需要检查的位定义（更正）

| 位 | 名称 | 正确值 |
|----|------|--------|
| sie bit 5 | STIE | 1 |
| sstatus bit 1 | SIE | 1 |
| sstatus bit 8 | SPP | 1 (S-mode) |
| mideleg bit 5 | STIP | 1 |

**更正**：你之前检查的位是错误的：
- `mideleg & 0x1` → 应该是 `mideleg & 0x20` (bit 5 = STIP)
- `medeleg & 0x10` → STIP 对应的异常委托是 bit 5

---

## 问题根源总结

### 当前问题
1. **错误的 CSR 读取**：读取了 S-mode 无权限访问的 medeleg/mideleg
2. **错误的位检查**：使用了错误的位掩码 (0x1, 0x10 而不是 0x2, 0x20)

### 根本原因
定时器中断没有到达 S-mode 的原因：
1. M-mode 没有正确设置 mideleg 委托 STIP
2. 或者 sstatus.SIE 没有启用

### 解决方案
1. 移除会触发 trap 的 medeleg/mideleg 读取代码
2. 在 M-mode 初始化中正确设置中断委托
3. 确保 sstatus.SIE = 1

---

## 下一步行动

1. **立即移除** main.rs 中的 medeleg/mideleg 读取代码
2. 检查 `tg-rcore-tutorial-sbi` 的 M-mode 初始化代码，确保设置了正确的 mideleg
3. 在 trap_handler 中添加更多调试输出，查看 scause 的原始值
