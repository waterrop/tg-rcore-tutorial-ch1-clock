# 架构分析：S-MODE 时钟中断驱动的定时字符串输出内核

## 1. 总览

本项目是一个 RISC-V 64 裸机内核最小闭环示例，运行在 QEMU `virt` 平台，采用 `-bios none` 启动模式：

- M-MODE：自带启动入口、最小陷阱处理与定时中断转发、最小 SBI 服务（console_putchar / set_timer / shutdown）。
- S-MODE：内核主体，建立 trap 框架，接收“转发后的”时钟中断，按秒输出字符串并统计耗时。

代码入口与模块组织：

- 启动入口（M）：[m_entry.asm](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/m_entry.asm)
- S 态入口与主循环：[main.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/main.rs)
- S 态 trap 与业务处理：[trap.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/trap.rs)
- 时间读取与换算：[timer.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/timer.rs)
- 链接脚本生成：[build.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/build.rs)
- QEMU runner 配置：[config.toml](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/.cargo/config.toml)

## 2. 关键设计点

### 2.1 为什么需要 M-MODE 入口

QEMU `-bios none` 不会提供 OpenSBI/RustSBI。CPU 复位后直接在 M 态从 0x8000_0000 执行。为了让 S 态内核具备“可运行的基本执行环境”，M 态需要完成：

- 配置特权级返回：`mstatus.MPP = S`，并设置 `mepc = _start`；
- 配置内存访问：PMP 放开对物理地址的访问；
- 配置计数器访问：`mcounteren` 允许 S 态读 `time`；
- 建立陷阱入口：设置 `mtvec = _m_trap`；
- 初始化定时器：写 `mtimecmp = mtime + interval`，打开 `mie.MTIE`；
- 提供最小 SBI：在 `_m_trap` 中处理 S-mode ecall（console_putchar / set_timer / shutdown）。

### 2.2 为什么需要“中断转发”

在 RISC-V 上，CLINT 产生的定时中断是 Machine Timer Interrupt（MTIP，异常号 7），不能直接委托给 S 态。常见做法是：

- M 态处理 MTIP 后，更新下一次 `mtimecmp`；
- 同时置位 `mip.STIP`，使 S 态看到 Supervisor Timer Interrupt Pending；
- S 态再以 Supervisor Timer Interrupt（异常号 5）触发自身的 trap 入口。

本项目在 M 态 `_m_trap` 中实现上述转发策略。

## 3. 启动与控制流

### 3.1 内存布局

由 [build.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/build.rs) 生成的链接脚本指定两段基地址：

- `0x8000_0000`：M 态区域（`.text.m_entry/.text.m_trap/.bss.m_stack/...`）
- `0x8020_0000`：S 态区域（`.text.entry` 为 `_start`，随后是 `.text/.rodata/.data/.bss`）

这保证 QEMU 从 M 态入口开始执行，并能跳转到固定的 S 态入口地址。

### 3.2 控制流（从上电到关机）

```text
QEMU (-bios none)
  └─ 0x8000_0000: _m_start (M-mode, m_entry.asm)
       ├─ 设置 mstatus.MPP=S, mepc=_start
       ├─ 配置 PMP / mcounteren
       ├─ mtvec=_m_trap
       ├─ 初始化 mtimecmp 并使能 MTIE
       └─ mret → 0x8020_0000: _start (S-mode, main.rs)
            ├─ 设置 S-mode 栈指针
            └─ rust_main
                 ├─ timer::get_time_ms() 记录起始时间
                 ├─ trap::init() 设置 stvec 并开中断
                 └─ loop { wfi }
                      │
                      │ 每秒：MTIP → _m_trap 处理并置 STIP
                      └→ STIP → s_trap_entry → trap_handler
                           ├─ set_timer(now + 1s)
                           ├─ 输出下一字符（console_putchar）
                           └─ 完成后输出 elapsed 并 shutdown
```

## 4. 模块职责与接口边界

### 4.1 main.rs：S 态入口与主循环

[main.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/main.rs) 负责：

- `_start`：裸机入口，设栈后跳转 `rust_main`；
- `rust_main`：记录起始时间、初始化 trap，然后进入 `wfi` 等待中断；
- `panic_handler`：异常收口，调用 `shutdown(true)`。

### 4.2 timer.rs：统一的时间基准

[timer.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/timer.rs) 负责：

- `read_time()`：读取 `time` CSR（`rdtime`），得到 tick；
- `get_time_ms()`：将 tick 转成毫秒；
- `CLOCK_FREQ`：统一的 tick 频率（用于换算与“1 秒”间隔）。

### 4.3 trap.rs：S 态 trap 与业务

[trap.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/trap.rs) 负责：

- `init(start_ms)`：
  - `stvec = s_trap_entry`；
  - 使能 `sie.STIE` 与 `sstatus.SIE`；
  - 初始化业务状态（输出索引、起始时间）。
- `s_trap_entry`：
  - 保存通用寄存器到栈（TrapFrame）；
  - 读取 `scause` 并调用 `trap_handler`；
  - 恢复寄存器并 `sret`。
- `trap_handler(scause)`：
  - 仅处理 Supervisor Timer Interrupt（异常号 5）；
  - 通过 `set_timer` 预约下一次中断；
  - 输出字符与换行；
  - 结束后打印耗时并关机。

### 4.4 m_entry.asm：M 态最小运行时与中断桥

[m_entry.asm](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/m_entry.asm) 负责两条关键路径：

- 启动路径 `_m_start`：M→S 的环境搭建与第一次定时器编程。
- 陷阱路径 `_m_trap`：
  - MTIP：更新 mtimecmp 并置位 STIP 转发给 S 态；
  - ecall：实现最小 SBI 服务（字符输出、设置定时器、关机）。

## 5. 运行时行为与可观测性

### 5.1 输出

输出通过 SBI `console_putchar` 逐字节写入串口。S 态每次中断输出一个字符并换行，便于肉眼观察“每秒一行”的节奏。

### 5.2 结束条件

字符串输出完成后，S 态打印耗时并调用 `shutdown(false)` 关机；这保证不会无限刷屏，也不会一直占用终端。

### 5.3 运行命令

```bash
timeout 10 cargo run
```

其中 `.cargo/config.toml` 提供 `cargo run` 的 QEMU runner 参数，隐藏了 `--target` 与 QEMU 启动命令的细节。

## 6. 可扩展方向

- 将“按秒输出”抽象为通用的 tick 驱动任务（tick → 任务调度）。
- 将 TrapFrame 结构化（用 Rust 结构体描述布局），并扩展对异常/其他中断的处理。
- 引入更精确的节拍策略（高频 tick + 业务分频），降低累计误差并提升可调度性。
