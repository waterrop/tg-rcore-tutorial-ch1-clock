# ch1-clock 时钟中断调试计划 (第四轮)

## 发现：mideleg 已正确设置

查看 `tg-rcore-tutorial-sbi/src/m_entry.asm`，发现 M-mode 初始化代码已经设置了：

```asm
# 第 26-27 行
li t0, 0xffff
csrw mideleg, t0
```

这意味着 `mideleg = 0xFFFF`，已经委托了所有中断（包括 STIP）到 S-mode。

**所以问题不在于中断委托设置！**

---

## 可能的新问题

### 原因分析

问题可能在于 **`set_timer` 的 SBI 实现**。

查看 `msbi.rs` 的 `handle_timer` 函数（第 139-156 行）：

```rust
fn handle_timer(time: u64) -> SbiRet {
    const CLINT_MTIMECMP: usize = 0x200_4000;

    // 1. 设置 mtimecmp
    unsafe {
        (CLINT_MTIMECMP as *mut u64).write_volatile(time);
    }

    // 2. 清除挂起的 STIP
    unsafe {
        asm!(
            "csrc mip, {}",
            in(reg) (1 << 5), // Clear STIP
        );
    }
    SbiRet::success(0)
}
```

**这里有个问题**：每次调用 `set_timer` 时，会先设置 `mtimecmp`，然后**清除** `mip.STIP`。

这本身是正确的行为（清除旧的中断请求），但问题是：
- 当定时器到期时，硬件会设置 `mip.STIP = 1`
- 如果此时 `sstatus.SIE = 0`（中断被屏蔽），中断不会触发

---

## 调试步骤

### 步骤 1: 确认 trap_handler 中的 scause 值

修改 `trap_handler`，输出 scause 的详细信息：

```rust
pub fn trap_handler() {
    let scause_val = scause::read().bits();

    console_putchar(b'x');

    // 输出 scause 的完整值（16 进制）
    // 格式：高位在前
    for i in (0..16).step_by(4) {
        let nibble = (scause_val >> (60 - i)) & 0xF;
        console_putchar(if nibble < 10 { b'0' + nibble as u8 } else { b'A' + (nibble - 10) as u8 });
    }

    // ... 后续处理
}
```

**预期输出格式**：
- 定时器中断：`0x8000000000000005`（高位到低位：8 0 0 0 0 0 0 0 0 0 0 0 0 0 0 5）
- 其他异常：查看具体值

### 步骤 2: 添加更多调试输出

在 `main.rs` 循环中，添加调试输出确认循环在运行：

```rust
loop {
    static mut COUNT: u32 = 0;
    unsafe {
        COUNT += 1;
        if COUNT % 10_000_000 == 0 {
            console_putchar(b'.');
        }
    }
    unsafe { core::arch::asm!("wfi"); }
}
```

---

## 另一种可能：M-mode 没有正确设置 mtimecmp

### 问题

在 QEMU virt 中，`mtimecmp` 的地址是 `0x2004000`，但可能需要考虑：
1. 正确的内存对齐
2. HART ID（对于多核）

### 检查方法

在 S-mode 读取 mtimecmp 验证（不可行，因为是内存映射）。

### 替代方案

在 `set_timer` 调用前后添加调试输出，验证调用是否成功：

```rust
let current_time = rdtime();
let next_time = current_time + interval;
set_timer(next_time);
// 如果 set_timer 成功返回，说明 SBI 调用成功
console_putchar(b'S');  // 表示 set_timer 完成
```

---

## 可能的解决方案

### 方案 A: 验证 sstatus.SIE 在 trap 返回后依然为 1

trap 处理完成后，`sret` 会恢复 sstatus。如果 SIE 位没有被正确保存/恢复，中断会被禁用。

检查 trap.S 中 sstatus 的处理：

```asm
# trap.S 中保存 sstatus
csrr t0, sstatus
sd t0, 32*8(sp)

# trap.S 中恢复 sstatus
ld t0, 32*8(sp)
csrw sstatus, t0
```

这段代码应该正确保存/恢复了 sstatus。

### 方案 B: trap handler 中手动设置 SIE

在 trap_handler 开始时强制启用中断：

```rust
pub fn trap_handler() {
    // 强制启用 SIE
    unsafe { core::arch::asm!("csrs sstatus, 2"); }

    // ... 原有代码 ...
}
```

---

## 完整调试代码

### trap_handler 修改

```rust
pub fn trap_handler() {
    // 强制启用 SIE
    unsafe { core::arch::asm!("csrs sstatus, 2"); }

    let scause_val = scause::read().bits();
    console_putchar(b'x');

    // 输出 scause 的完整值
    for i in (0..16).step_by(4) {
        let nibble = (scause_val >> (60 - i)) & 0xF;
        console_putchar(if nibble < 10 { b'0' + nibble as u8 } else { b'A' + (nibble - 10) as u8 });
    }

    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            console_putchar(b'b');
            let interval = 10_000_0u64;
            let current_time = rdtime();
            set_timer(current_time + interval);
        }
        _ => {
            console_putchar(b'p');
            panic!("trap: {:?}", scause.cause());
        }
    }
}
```

### main.rs 修改

```rust
loop {
    static mut COUNT: u32 = 0;
    unsafe {
        COUNT = COUNT.wrapping_add(1);
        if COUNT % 5_000_000 == 0 {
            console_putchar(b'.');
        }
    }
    unsafe { core::arch::asm!("wfi"); }
}
```

---

## 下一步行动

1. 在 trap_handler 中添加 scause 完整值输出
2. 在循环中添加 `.` 计数器，确认程序在运行
3. 运行测试，观察输出
4. 根据 scause 值判断问题

如果 trap handler 完全没有被调用，则问题在中断传递链；如果被调用了但 scause 不是定时器中断，则需要进一步分析。
