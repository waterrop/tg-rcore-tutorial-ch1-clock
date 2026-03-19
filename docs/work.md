# 工作过程记录：S-MODE 时钟中断定时输出 hello world

## 1. 目标回顾

在 `tg-rcore-tutorial-ch1-clock` 的最小裸机内核基础上，完成以下能力：

- 在 S-MODE 建立时钟中断处理机制（stvec + s_trap_entry + handler）。
- 每隔约 1 秒输出一个字符，最终输出完 `hello world`。
- 输出完成后打印总耗时，并关机退出。

对应设计输入为 [design.md](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/design.md)。

## 2. 实施过程（按时间顺序）

### 2.1 搭建可运行的启动链路（M-mode -> S-mode）

核心判断：S 态程序要能“跑起来并输出”，必须保证：

- QEMU `-bios none` 启动时从 0x8000_0000 的 M 态入口开始执行；
- M 态设置好 mstatus/mepc/pmp/mcounteren/mtvec/定时器后 `mret` 进入 S 态；
- S 态 `_start` 设栈后进入 Rust 的 `rust_main`；
- 输出路径要可用（通过 SBI 的 console_putchar / shutdown）。

对应落地：

- 增补二进制与构建脚本配置：在 [Cargo.toml](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/Cargo.toml) 中加入 `build = "build.rs"` 与 `[[bin]]`。
- 引入 M 态入口汇编：新增 [m_entry.asm](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/m_entry.asm)（负责 M 态初始化、时钟中断转发、SBI ecall 处理）。
- 使用构建脚本生成链接脚本：复用 [build.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/build.rs) 在 riscv64 目标下生成 `linker.ld`，将 `.text.m_entry` 放到 0x8000_0000，S 态 `.text.entry` 放到 0x8020_0000。
- 保持 runner 配置：通过 [.cargo/config.toml](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/.cargo/config.toml) 在 `cargo run` 时自动调用 QEMU。

### 2.2 打通 S-MODE Trap 框架

S 态时钟中断的关键链路是：

- CLINT 触发 Machine Timer Interrupt（MTIP），硬件陷入 M 态 `_m_trap`；
- M 态更新 mtimecmp 并置位 `mip.STIP`（把中断“转发”为 S 态可见的 STIP）；
- S 态看到 `sip.STIP=1` 后陷入 `stvec` 指向的入口；
- `s_trap_entry` 保存上下文并调用 Rust 的 `trap_handler`；
- `trap_handler` 通过 `set_timer` 设置下一次中断时间，并完成输出。

对应落地：

- 在 [trap.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/trap.rs) 中实现：
  - `init(start_ms)`：设置 `stvec`，使能 `sie.STIE` 与 `sstatus.SIE`。
  - `s_trap_entry`：裸函数，保存/恢复通用寄存器并 `sret` 返回。
  - `trap_handler`：识别 Supervisor Timer Interrupt（exception code = 5），驱动输出与关机。

### 2.3 实现“每秒输出一字符”的业务逻辑

业务状态最小化实现为两个静态变量：

- `CHAR_IDX`：下一次输出的字符索引
- `START_TIME_MS`：启动时刻（毫秒）

每次时钟中断：

1. 先调用 `set_timer(now + CLOCK_FREQ)` 预约下一次中断，确保节拍连续；
2. 输出 `HELLO_WORLD[CHAR_IDX]` 并换行；
3. 若已输出完 11 个字符，计算耗时并打印 `elapsed: X s Y ms`，然后 `shutdown(false)`。

时间读取与换算放在 [timer.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/timer.rs)，通过 `rdtime` 读取 time CSR 并按 `ticks * 1000 / CLOCK_FREQ` 转为毫秒。

### 2.4 运行验证

运行命令（避免卡住终端）：

```bash
timeout 10 cargo run
```

预期现象：

- QEMU 启动后，约每秒输出一行字符；
- 依次输出 `hello world`（11 行）；
- 最后额外输出一行 `elapsed: ...` 并关机退出。

## 3. 遇到的问题与解决过程

### 3.1 项目配置差异导致“能编译但跑不起来/不输出”

现象：

- `cargo run` 能编译，但运行阶段行为异常（例如没有进入预期入口、没有输出）。

定位要点：

- 裸机程序需要链接脚本把入口段放到 QEMU 期望的地址；
- 需要 M 态入口负责切到 S 态，并提供最小 SBI 服务（输出/关机/定时器）。

解决：

- 补齐 `build.rs` 的链接脚本接入（Cargo.toml 的 `build = "build.rs"`）；
- 补齐 `[[bin]]` 指定二进制入口；
- 引入 M 态入口汇编与 S 态 `_start` 入口的正确连接。

### 3.2 S 态无法直接清除 STIP（中断“清不掉”）

现象：

- 即使在 S 态 handler 里执行 `csrc sip, (1<<5)`，也可能无法稳定清除 STIP，导致重复陷入或行为不符合预期。

原因：

- `sip.STIP` 的置位来自 M 态转发，S 态对该 pending 位并非总是可写清；
- 正确做法是通过 SBI `set_timer` 进入 M 态，由 M 态在实现中清理转发相关状态（例如清 `mip.STIP`）。

解决：

- 在 S 态 handler 中通过 `set_timer` 完成“下一次中断设置 + 清理相关 pending”。

### 3.3 CSR 立即数限制导致汇编设置 sie 失败

现象：

- 试图使用 `csrsi sie, 0x20` 使能 STIE（bit 5）时，可能出现“立即数超范围”或汇编失败。

原因：

- `csrsi/csrci` 的立即数只有 5 位（0~31），而 `0x20` 是 32。

解决：

- 使用寄存器形式：`li t0, 0x20; csrs sie, t0`。

### 3.4 平台时钟频率不一致导致“每秒”不准

现象：

- 输出间隔明显不是 1 秒。

原因：

- QEMU virt 的 CLINT 频率常用配置是 12.5MHz（示例按 12_500_000 ticks/s），若代码用错常量会导致节拍偏差。

解决：

- 在 [timer.rs](file:///home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/src/timer.rs) 中统一使用 `CLOCK_FREQ = 12_500_000` 并用于换算与定时。

### 3.5 本环境下串口输出可见性问题（工具链/运行器差异）

现象：

- `timeout 10 cargo run` 显示 QEMU 启动命令，但控制台可能看不到串口字符输出。

可能原因：

- runner/沙箱对 QEMU 标准输入输出的重定向策略与本地终端不同；
- `-nographic` 下串口输出依赖 `-serial` 配置与宿主终端支持方式。

应对：

- 保持 `.cargo/config.toml` 的 runner 参数可调（例如增加 `-serial mon:stdio`）；
- 在真实本地环境直接运行等价 qemu 命令进行观察验证。

## 4. 结果与验收对照

- 已具备 M-mode 入口初始化、S-mode trap 框架、时钟中断驱动的定时输出、耗时统计与关机闭环。
- 代码组织与设计文档的模块划分一致：`main.rs + m_entry.asm + trap.rs + timer.rs + build.rs`。
