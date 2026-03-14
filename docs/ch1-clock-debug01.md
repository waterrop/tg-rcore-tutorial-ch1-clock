# ch1-clock 时钟中断调试计划

## 调试环境

当前代码运行情况：
- 程序启动后输出 "Trap handling started!" 和 "S-Mode started!" 和 "Time started!"
- 但之后没有任何 'b' 或 'x' 输出，说明 trap handler 没有被调用

---

## 调试步骤与原因分析

### 步骤 1: 验证中断是否触发（添加调试输出）

**操作**：在 `rust_main` 循环中添加 wfi + 调试输出

**原因**：确认 CPU 是否在等待中断，以及中断是否到达 S-mode

```rust
loop {
    // 添加 wfi 降低功耗，同时检测中断
    unsafe { core::arch::asm!("wfi"); }
    console_putchar(b'.');  // 如果执行到这里，说明有东西在运行
}
```

**期望现象**：
- 如果看到连续的 '.'，说明循环在运行，但中断没来
- 如果看到 'b'，说明 trap handler 被调用了

---

### 步骤 2: 检查 sstatus 和 sie CSR

**操作**：在循环中添加 CSR 状态读取

```rust
use riscv::register::{sstatus, sie};

loop {
    unsafe {
        core::arch::asm!(
            "csrr {0}, sstatus",
            "csrr {1}, sie",
            out(reg) sstatus_val,
            out(reg) sie_val
        );
    }
    // 输出这两个值
    console_putchar(b'S');
    // ... 输出 sstatus_val 和 sie_val ...
    unsafe { core::arch::asm!("wfi"); }
}
```

**需要检查的位**：
- `sie` 中的 `STIE` (bit 5) 是否为 1：启用 S-mode 定时器中断
- `sstatus` 中的 `SIE` (bit 1) 是否为 1：全局中断启用
- `sstatus` 中的 `SPP` (bit 8)：Previous Privilege Mode，应为 1 (S-mode)

---

### 步骤 3: 检查 scause 是否被设置

**操作**：在循环中读取 scause

```rust
use riscv::register::scause;

loop {
    let cause: usize;
    unsafe { core::arch::asm!("csrr {0}, scause", out(reg) cause); }
    if cause != 0 {
        // 输出 cause 值
    }
    unsafe { core::arch::asm!("wfi"); }
}
```

**原因**：如果 scause 不为 0，说明之前有 trap 发生过

---

### 步骤 4: 检查 M-mode 中断委托配置

**操作**：检查 medeleg 和 mideleg CSR

```rust
let medeleg: usize;
let mideleg: usize;
unsafe {
    core::arch::asm!("csrr {0}, medeleg", out(reg) medeleg);
    core::arch::asm!("csrr {0}, mideleg", out(reg) mideleg);
}
```

**需要检查的位**：
- `mideleg` 的 bit 5 (STIP) 应为 1：将定时器中断委托给 S-mode
- `mideleg` 的 bit 1 (SSIP) 应为 1：将软件中断委托给 S-mode

---

### 步骤 5: 检查 sscratch 初始化

**问题原因分析**：

trap.S 第 13 行：
```asm
csrrw sp, sscratch, sp
```

这行代码假设 sscratch 已经被初始化为用户栈地址。但在 ch1 中：
- 没有用户态程序
- sscratch 可能没有被初始化（值为 0）
- 这会导致栈交换到地址 0，产生错误

**操作**：在 `init()` 中初始化 sscratch

```rust
pub fn init() {
    // 打印消息...

    // 初始化 sscratch 为 0（表示内核态）
    unsafe {
        core::arch::asm!("csrw sscratch, zero");
    }

    // 设置 stvec...
}
```

或者修改 trap.S，在进入时检查 sstatus.SPP：

```asm
__alltraps:
    csrr t0, sstatus
    andi t0, t0, 0x100  # 检查 SPP 位
    bnez t0, _from_kernel  # 如果 SPP=1，跳过交换

_from_user:
    csrrw sp, sscratch, sp
    j _continue

_from_kernel:
    # 不交换栈
    addi sp, sp, -34*8
    j _continue

_continue:
    # 继续保存寄存器...
```

---

## 可能的问题原因及解决方案

### 问题 1: sscratch 未初始化

**现象**：程序卡死，无任何输出

**原因**：trap.S 使用 `csrrw sp, sscratch, sp` 交换栈，但 sscratch = 0，导致栈被交换到地址 0

**解决**：
```rust
pub fn init() {
    unsafe {
        core::arch::asm!("csrw sscratch, zero");
    }
    // ...
}
```

---

### 问题 2: 中断未正确委托

**现象**：无 'x' 输出，说明 trap 没有发生

**原因**：M-mode 没有将定时器中断委托给 S-mode

**解决**：在 M-mode 初始化中设置 mideleg。检查 `tg-rcore-tutorial-sbi` 是否正确设置了：
```rust
// 应该在 M-mode 启动代码中添加
csrw mideleg, zero
# 或者
csrs mideleg, 0x22  # 委托 STIP 和 SSIP
```

---

### 问题 3: sstatus.SIE 未启用

**现象**：无 'x' 输出

**原因**：sstatus 中的全局中断启用位 SIE = 0

**解决**：在 enable_timer_interrupt 或 init 中确保 SIE = 1
```rust
unsafe {
    core::arch::asm!("csrs sstatus, 2");  # 设置 SIE 位
}
```

---

### 问题 4: sstatus.SPP 设置错误

**现象**：trap 返回后权限模式错误

**原因**：进入 trap 时 sstatus.SPP 应设置为 1 (S-mode)

**解决**：在 trap.S 中保存正确的 sstatus：
```asm
# 保存之前，确保 SPP = 1
csrr t0, sstatus
ori t0, t0, 0x100  # 设置 SPP = 1
csrw sstatus, t0
```

---

### 问题 5: stvec 设置错误

**现象**：程序跳转到了错误地址

**原因**：stvec 设置的地址不正确，或者使用了 Vectored 模式但地址未对齐

**解决**：确保使用 Direct 模式且地址对齐：
```rust
stvec::write(__alltraps as usize, TrapMode::Direct);
```

---

## 推荐调试顺序

1. **第一步**：添加 wfi + '.' 输出，确认循环在运行
2. **第二步**：读取 sstatus/sie，确认中断已启用
3. **第三步**：初始化 sscratch = 0
4. **第四步**：检查 M-mode 中断委托
5. **第五步**：逐步执行 trap.S，确认每条指令正确

---

## 快速验证清单

| 检查项 | 预期值 | 检查位置 |
|--------|--------|----------|
| sie.STIE | 1 | sie CSR |
| sstatus.SIE | 1 | sstatus CSR |
| sstatus.SPP | 1 | sstatus CSR |
| mideleg.STIP | 1 | mideleg CSR |
| stvec | 指向 __alltraps | stvec CSR |
| sscratch | 0 | sscratch CSR |
