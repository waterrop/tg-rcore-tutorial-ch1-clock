# ch1-clock 时钟中断调试计划 (第五轮)

## 问题分析

用户反馈：**完全没有输出 'x'**，说明 trap_handler 根本没有被调用。

---

## 回答问题 1-3：为什么没有进入 trap_handler？

### 回答：不是进入 m_trap_handler

**原因**：

1. `m_trap_handler` 只处理 `mcause == 9`（S-mode ecall）
2. 定时器中断的 `mcause == 5`，不满足这个条件
3. 如果真的进入了 M-mode trap，会返回 `SbiRet::not_supported()`，导致程序行为异常而不是正常等待

**真正的可能原因**：

### 原因 1: mstatus.MIE 未启用（M-mode 全局中断未启用）

这是**最可能的原因**！

在 `m_entry.asm` 中，启动代码没有显式设置 `mstatus.MIE`（Machine Interrupt Enable）：

```asm
# 第 12-14 行
li t0, (1 << 11) | (1 << 7)   # MPP=01, MPIE=1
csrw mstatus, t0
```

这里设置了：
- `MPP = 01` → 返回到 S-mode
- `MPIE = 1` → 启用 M-mode 中断（当进入低特权级时）

但没有设置 `MIE (bit 3)`！

**如果 MIE = 0，M-mode 自己的中断被屏蔽**，定时器中断无法传递！

---

## 回答问题 4：解决方案

### 修复方案：在 m_entry.asm 中启用 MIE

在 `m_entry.asm` 的 M-mode 初始化中添加：

```asm
# 启用 M-mode 全局中断
li t0, (1 << 3)   # MIE = 1
csrs mstatus, t0
```

或者修改现有的 mstatus 设置：

```asm
# 修改第 12-14 行
# 原来：
li t0, (1 << 11) | (1 << 7)
# 改为：
li t0, (1 << 11) | (1 << 7) | (1 << 3)  # MPP=01, MPIE=1, MIE=1
```

---

## 回答问题 5：其他可能原因及解决方案

### 原因 2: stvec 设置不正确

**可能**：stvec 设置的地址有问题。

**验证方法**：在 init() 中添加 stvec 输出：

```rust
pub fn init() {
    for c in b"Trap handling started!\n" {
        console_putchar(*c);
    }

    // 输出 stvec 值验证
    let stvec_val: usize;
    unsafe { core::arch::asm!("csrr {0}, stvec", out(reg) stvec_val); }
    // 输出 stvec...

    unsafe extern "C" {
        fn __alltraps();
    }

    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }

    // 再次输出验证
    unsafe { core::arch::asm!("csrr {0}, stvec", out(reg) stvec_val); }
    // 输出...
}
```

### 原因 3: set_timer 没有正确设置 mtimecmp

**可能**：SBI set_timer 调用没有成功设置硬件定时器。

**验证方法**：在 `main.rs` 中验证 set_timer 调用：

```rust
set_timer(next_time);
console_putchar(b'T');  // 如果执行到这里，说明 set_timer 返回了

// 验证：读取 mtime（需要知道地址）
const CLINT_MTIME: usize = 0x200_bff8;
let mtime: u64;
unsafe { mtime = (CLINT_MTIME as *const u64).read_volatile(); }
// 输出 mtime 和 next_time 比较
```

### 原因 4: trap.S 中 __alltraps 执行出错

**可能**：trap.S 汇编代码执行出错（如栈交换失败）。

**验证方法**：简化 trap.S，先不用复杂逻辑：

```asm
__alltraps:
    # 简单的 trap 入口，不交换栈
    addi sp, sp, -64
    # 保存必要寄存器
    sd ra, 0(sp)
    sd t0, 8(sp)
    # 调用 trap_handler
    mv a0, sp
    call trap_handler
    # 恢复并返回
    ld ra, 0(sp)
    ld t0, 8(sp)
    addi sp, sp, 64
    sret
```

### 原因 5: QEMU 参数问题

**可能**：QEMU 没有正确传递时钟中断。

**验证方法**：检查 QEMU 启动参数是否有问题。当前 `.cargo/config.toml` 中的配置应该没问题。

---

## 完整调试计划

### 步骤 1: 修改 m_entry.asm，启用 MIE

这是**最可能的修复方案**：

```asm
# 在 m_entry.asm 第 12-14 行，修改为：
li t0, (1 << 11) | (1 << 7) | (1 << 3)  # MPP=01, MPIE=1, MIE=1
csrw mstatus, t0
```

### 步骤 2: 重新编译运行

```bash
cd tg-rcore-tutorial-ch1-clock
cargo clean
cargo run
```

观察是否输出 'x' 或 'b'。

### 步骤 3: 如果依然不行，添加更多调试输出

1. 在 trap_handler 入口添加更多输出
2. 在 trap.S 中添加调试输出

---

## 总结

| 问题 | 可能性 | 解决方案 |
|------|--------|----------|
| mstatus.MIE 未启用 | **最高** | 修改 m_entry.asm 添加 MIE=1 |
| stvec 设置错误 | 中等 | 添加调试输出验证 |
| set_timer 未生效 | 中等 | 验证 mtimecmp |
| trap.S 执行错误 | 低 | 简化 trap.S |
| QEMU 配置问题 | 低 | 检查参数 |
