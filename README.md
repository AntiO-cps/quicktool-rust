# QuickTool Rust

原生 Rust 重写工程，分支 `codex/rust-rewrite`。当前仍在迁移，尚未满足全部 UI、交互一致性要求。

## 运行

需要 Rust 与 Windows MSVC 工具链。首次构建需要下载 Cargo 依赖。

```powershell
cargo run --release
```

运行时使用 Rust 原生窗口和绘制，不运行 Electron、Node.js 或参考 JavaScript。原版字体、图标、国旗与数据保留在 `assets/`。

字体已嵌入程序。国旗等资源从可执行文件旁的 `assets/` 加载。

生成可移植目录：`./scripts/Build-Rust.ps1 -Offline`。运行 `dist/QuickTool.exe`，并保留旁边的资源目录。

数据默认写入 `%APPDATA%/QuickTool-Rust`。测试可用 `QUICKTOOL_RUST_DATA_DIR` 指定隔离数据目录。原版 QuickTool 数据暂不自动导入。

## 验证

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
```

`reference/` 仅用于迁移对照；`scripts/capture-baseline.cjs` 从原版提取差分用例。测试直接比较 Rust 与原版的输出，不以 Rust 自身计算结果作为预期值。

文件系统通知集成测试使用随机生成的临时文件。受限 Windows 沙箱可能禁止重命名；需在具备正常临时目录权限的环境运行。

## 性能证据

[像素基准](docs/pixel-benchmark.json)：同一输入与滤镜参数，1,048,576 像素，预热 3 次、采样 7 次。当前中位耗时 JS 88.93 ms，Rust Release 54.80 ms（约 1.62 倍），结果校验一致。此结果仅说明像素计算性能，不能代表整机启动、内存、搜索或拖拽性能。

[应用基准](docs/app-benchmark.json)：同机三轮隔离采样，启动中位数 2.445 s → 0.699 s，空闲工作集约 855 MiB → 194 MiB。启动计时从应用入口开始，不包括 OS 创建进程；原版内存未包含外部辅助进程。多数查询 CPU 重建更快，但首次 Unicode 查询仍较慢，且不同渲染器的布局耗时不能直接视为同等交互延迟。大目录索引与拖拽帧耗时尚未验证。

## 状态

见 [持续工作记录](AGENT_WORKLOG.md) 与 [迁移验收清单](docs/migration.md)。
