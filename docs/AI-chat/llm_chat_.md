# question1
扩展此内核功能，支持在S-MODE的时钟中断机制，可以定时显示字符串，比如每隔一秒显示一个字符，最终显示完毕“hello world”，以及显示出显示完毕的总耗时。在完成此项目前，形成工作计划的设计文档（保存在/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/design.md），在完成项目后，形成工作过程的描述文档，包括碰到的问题，以及解决的过程（保存在/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/work.md）。最后，写出整个项目的架构分析文档（保存在/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/arch.md）。给出计划，我确认后，完成此工作。

# res1
AI先给出了一个简单的设计方案。

# question3
为设计方案添加规划如下，修改design.md文档
确定不修改 `tg-rcore-tutorial-sbi` 共享库，由 `ch1-clock` 自带完整的 M-mode 入口代码（`src/m_entry.asm`）。设计方案如下：

- `Cargo.toml`：去掉 `nobios` feature，让 `tg-sbi` 只作为 SBI 调用封装
- `src/m_entry.asm`：新建，包含 M-mode 启动代码和时钟中断转发逻辑
- `src/timer.rs`：新建，提供 `read_time()` 和 `get_time_ms()` 工具函数
- `src/trap.rs`：新建，S-mode 陷阱处理框架（stvec 设置 + 中断分发 + 字符输出）
- `src/main.rs`：修改，嵌入 `m_entry.asm`，启动后调用 `trap::init` 并 `wfi` 等待

# question4
按照/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/design.md实现代码。

# question5
实现代码后，输入cargo run没有输出hello world，也没有显示总耗时，一直在空转，请排查bug的原因并修改。

# question6
把/home/hdu/study/rust/tg-rcore-tutorial-ch1-clock称为项目A
把/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/称为项目B
对比项目A和项目B，告诉我为什么项目A可以输出hello world，而项目B不能输出hello world
并按照项目A的实现，修改项目B的代码，使项目B也可以输出hello world，并生成新的代码实现计划（保存在/home/hdu/study/rust/2026s-ai4ose-lab/tg-rcore-tutorial-ch1-clock/docs/design02.md），我确认后，按照新的计划实现项目B。