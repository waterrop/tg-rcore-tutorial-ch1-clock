# ch1-clock 时钟中断调试计划 (第三轮)

## 当前状态

- trap handler 入口已正确设置（stvec 配置正确）
- 删除 medeleg/mideleg 代码后，程序可以正常运行
- 但**时钟中断从未触发**

---

## 时钟中断未触发的可能原因

### 原因 1: sstatus.SIE 未启用（全局中断开关）

**原理**：sstatus.SIE 是 S-mode 的全局中断启用位。如果为 0，所有 S-mode 中断都会被屏蔽。

**检查方法**：在 `enable_timer_interrupt()` 中添加：

```rust
pub fn enable_timer_interrupt() {
    for c in b"S-Mode started!\n" {
        console_putchar(*c);
    }

    // 检查 sstatus
    let sstatus_val: usize;
    unsafe { core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus_val); }

    // 输出 sstatus 值（只输出低8位）
    for i in 0..8 {
        console_putchar(b'0' + ((sstatus_val >> (i*4)) & 0xF) as u8);
    }

    // 检查 SIE 位 (bit 1)
    if sstatus_val & 0x2 == 0 {
        // SIE 未启用，手动设置
        unsafe { core::arch::asm!("csrs sstatus, 2"); }
        console_putchar(b'E');  // 表示手动启用了 SIE
    } else {
        console_putchar(b'K');  // 表示 SIE 已启用
    }

    unsafe {
        sie::set_stimer();
    }
}
```

**预期**：
- 如果输出包含 'E'，说明 SIE 之前是 0，现在已手动启用
- 如果输出包含 'K'，说明 SIE 已经是 1

---

### 原因 2: sie.STIE 未启用（定时器中断开关）

**原理**：即使 sstatus.SIE = 1，如果 sie.STIE = 0，定时器中断仍然被屏蔽。

**检查方法**：在 `enable_timer_interrupt()` 中添加：

```rust
pub fn enable_timer_interrupt() {
    // ... 上面的代码 ...

    // 检查 sie
    let sie_val: usize;
    unsafe { core::arch::asm!("csrr {0}, sie", out(reg) sie_val); }

    // 输出 sie 值
    for i in 0..8 {
        console_putchar(b'0' + ((sie_val >> (i*4)) & 0xF) as u8);
    }

    // 检查 STIE 位 (bit 5)
    if sie_val & 0x20 == 0 {
        // STIE 未启用，手动设置
        unsafe { core::arch::asm!("csrs sie, 0x20"); }
        console_putchar(b'T');  // 表示手动启用了 STIE
    } else {
        console_putchar(b'A');  // 表示 STIE 已启用
    }
}
```

---

### 原因 3: set_timer 未正确设置时间

**原理**：即使中断启用，如果没有设置下一次中断触发时间，定时器中断不会发生。

**检查方法**：在设置定时器后添加验证：

```rust
// 设置第一次定时器中断
let interval = 10_000_000u64;
let current_time = rdtime();
let next_time = current_time + interval;
set_timer(next_time);

// 验证：读取 mtimecmp（通过 SBI 或直接读取）
// 注意：mtimecmp 是内存映射的，需要通过地址读取
```

**输出调试信息**：
```rust
for c in b"Time started!\n" {
    console_putchar(*c);
}

// 输出 next_time 的值用于调试
for i in 0..16 {
    let nibble = (next_time >> (60 - i*4)) & 0xF;
    console_putchar(if nibble < 10 { b'0' + nibble as u8 } else { b'A' + (nibble - 10) as u8 });
}
```

---

### 原因 4: M-mode 中断委托未设置

**原理**：如果 M-mode 没有将 STIP 委托给 S-mode（mideleg.STIP = 0），定时器中断会在 M-mode 处理，不会传递到 S-mode。

**问题**：无法直接从 S-mode 读取 mideleg。

**解决方案**：检查 `tg-rcore-tutorial-sbi` 的 M-mode 初始化代码，确保设置了：

```asm
# 在 M-mode 初始化中设置
csrw mideleg, zero
# 或
csrs mideleg, 0x22  # 委托 STIP(5) 和 SSIP(1)
```

---

### 原因 5: CLINT 未正确配置

**原理**：`set_timer` 调用 SBI 设置 mtimecmp，如果 SBI 实现有问题，时间可能没有正确设置。

**检查方法**：
1. 使用更短的时间间隔测试（如 1,000,000 tick）
2. 在循环中添加 wfi 降低 CPU 功耗，让中断更容易触发

---

## 调试步骤总结

### 步骤 1: 启用 sstatus.SIE

```rust
pub fn enable_timer_interrupt() {
    // 强制启用 S-mode 全局中断
    unsafe { core::arch::asm!("csrs sstatus, 2"); }
    console_putchar(b'S');

    // 启用定时器中断
    unsafe { sie::set_stimer(); }
    console_putchar(b'T');
}
```

### 步骤 2: 添加 wfi 降低功耗

```rust
loop {
    // Wait For Interrupt - 降低功耗，等待中断
    unsafe { core::arch::asm!("wfi"); }
    // 如果中断发生，会跳转到 trap handler
    // 如果没中断，程序继续执行
}
```

### 步骤 3: 检查时间设置

- 确认 `set_timer` 被调用
- 确认 `next_time` 值合理（当前时间 + interval）
- 尝试缩短 interval 到 1,000,000 或更小

### 步骤 4: 验证 M-mode 委托

检查 `tg-rcore-tutorial-sbi/src/msbi.rs` 或相关 M-mode 代码：

```rust
// 应该包含类似这样的代码
csrw mideleg, 0x22  // 委托 STIP 和 SSIP
```

---

## 快速测试方案

### 最小测试代码

```rust
extern "C" fn rust_main() -> ! {
    trap::init();

    // 强制启用所有中断
    unsafe {
        core::arch::asm!("csrs sstatus, 2");  // SIE = 1
        core::arch::asm!("csrs sie, 0x20");   // STIE = 1
    }
    console_putchar(b'I');  // 中断已启用

    // 设置定时器
    let now = rdtime();
    set_timer(now + 5_000_000);  // 5秒后
    console_putchar(b'T');  // 定时器已设置

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
```

预期输出：`Trap handling started!Time started!I T`（然后等待中断）

---

## 检查清单

| 检查项 | 预期 | 操作 |
|--------|------|------|
| sstatus.SIE (bit 1) | 1 | 手动 `csrs sstatus, 2` |
| sie.STIE (bit 5) | 1 | 使用 `sie::set_stimer()` |
| set_timer | 被调用 | 添加调试输出 |
| mideleg.STIP (M-mode) | 1 | 检查 sbi 初始化代码 |

---

## 下一步行动

1. 修改 `enable_timer_interrupt()` 强制启用 sstatus.SIE
2. 在循环中添加 wfi
3. 缩短 interval 到 5,000,000 或更小
4. 运行测试，观察是否有 'b' 输出
