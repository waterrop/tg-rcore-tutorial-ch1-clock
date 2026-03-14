# ch1-clock 时钟中断实现计划

## 背景与问题

当前 ch1-clock 的实现使用**轮询方式**检测时钟中断：
- 在 `rust_main` 的 `loop` 中不断读取 `scause` CSR 来判断是否发生定时器中断
- 这不是真正的中断处理，而是 busy loop

用户希望实现真正的时钟中断机制。

## 问题解答

### 1. 是否需要实现 trap 上下文？

**不需要完整的 trap 上下文保存/恢复**，因为：
- ch1 是最小裸机程序，只需处理时钟中断这一种中断源
- trap handler 只需保存少量必要寄存器（ra, t0 等），无需完整的上下文切换
- 系统调用（ecall）不是 ch1 的目标，那是 ch2 的内容

### 2. 是否需要实现系统调用？

**不需要**。ch1 的目标是：
- 理解最小执行环境
- 演示时钟中断的基本机制
- 不需要用户态/系统调用

## 修改方案

### 核心修改点

#### 1. 启用 S-mode 定时器中断 (已实现)
```rust
unsafe { sie::set_stimer() };
```

#### 2. 设置 stvec CSR 指向 trap handler
在 `rust_main` 开头设置：
```rust
unsafe {
    // 设置 stvec 指向 trap handler (Direct 模式)
    stvec::write(s_trap_handler as usize, stvec::TrapMode::Direct);
}
```

#### 3. 编写 trap handler
```rust
#[unsafe(no_mangle)]
extern "C" fn s_trap_handler() {
    // 读取 scause 判断中断类型
    let scause = scause::read().bits() as usize;
    // 判断是否为 S-mode 定时器中断 (STIP = 5)
    if scause == 0x8000000000000005 {
        // 处理时钟中断
        // ...
    }
    // 清除挂起的定时器中断
    unsafe { sip::clear_ssip() };
    // 返回原程序
    core::arch::naked_asm!("sret");
}
```

#### 4. 移除轮询代码
删除 `loop` 中轮询 `scause` 的代码。

### 需要修改的文件

| 文件 | 修改内容 |
|------|----------|
| `src/main.rs` | 1. 导入 `riscv::register::{scause, stvec, sip}`<br>2. 设置 `stvec` 指向 trap handler<br>3. 编写 `s_trap_handler` 函数<br>4. 删除轮询代码 |

### 详细实现

#### 步骤 1: 添加必要的 CSR 导入
```rust
use riscv::register::{scause, sie, sip, stvec};
```

#### 步骤 2: 设置 trap handler 地址
在 `rust_main()` 函数开头添加：
```rust
// 设置 stvec 指向 trap handler，使用 Direct 模式
unsafe {
    stvec::write(s_trap_handler as usize, stvec::TrapMode::Direct);
}
```

#### 步骤 3: 编写 trap handler
```rust
#[unsafe(no_mangle)]
extern "C" fn s_trap_handler() {
    let scause_val = scause::read().bits();
    // 最高位为 1 表示中断，低位 5 表示 S-mode 定时器中断
    if scause_val == 0x8000000000000005 {
        // 处理时钟中断
        console_putchar(b't');
        // 设置下一次中断
        let interval = 10_000_000u64;
        let current = rdtime();
        set_timer(current + interval);
    }
    // 清除挂起的定时器中断 (STIP)
    unsafe { sip::clear_ssip() };
    // 使用 sret 返回
    core::arch::naked_asm!("sret");
}
```

#### 步骤 4: 简化主循环
```rust
loop {
    // 真正的中断驱动，无需轮询
    // 可以添加 wfi 指令降低功耗（可选）
    unsafe { core::arch::asm!("wfi"); }
}
```

## 验证方法

运行 `cargo run` 后应看到：
1. 输出 "Time started!"
2. 每秒输出一个 't' 字符
3. 10 次后输出 "Shutdown!" 并退出

## 注意事项

1. **nobios 模式**：当前使用 `nobios` 特性，M-mode 固件已经设置好中断委托，STIP 会被传递到 S-mode
2. **栈空间**：trap handler 需要足够的栈空间，当前 4KB 栈足够
3. **riscv crate**：使用已有的 `riscv` crate 来访问 CSR，比内联汇编更安全

## 总结

- **不需要**实现完整的 trap 上下文保存
- **不需要**实现系统调用
- 只需：
  1. 设置 `stvec` 指向 trap handler
  2. 编写简单的 trap handler 处理定时器中断
  3. 使用 `sret` 返回
  4. 删除轮询代码
