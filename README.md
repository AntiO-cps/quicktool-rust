# QuickTool Rust

原生 Rust 重写工程，分支 `codex/rust-rewrite`。当前仍在迁移，尚未满足全部 UI、交互一致性要求。

## 运行

需要 Rust 与 Windows MSVC 工具链。首次构建需要下载 Cargo 依赖。

```powershell
cargo run --release
```

运行时使用 Rust 原生窗口和绘制，不运行 Electron、Node.js 或参考 JavaScript。原版字体、图标、国旗与数据保留在 `assets/`。

字体已嵌入程序。国旗等资源从可执行文件旁的 `assets/` 加载。

生成可移植目录：`./scripts/Build-Rust.ps1 -Offline`。运行 `dist/QuickTool.exe`，并保留旁边的资源与许可目录。依赖许可清单在 `licenses/rust/INDEX.json`。

安装包位于 `releases/QuickTool-Rust-Setup-0.6.0-alpha.33-x64.exe`，旁边提供 SHA256 校验文件。默认安装到当前用户的 `LocalAppData/Programs/QuickTool Rust`，创建快捷方式及卸载入口；卸载保留设置和历史。

使用已有 NSIS 编译器生成安装包：`./scripts/Build-Installer.ps1 -MakeNsis PATH_TO_MAKENSIS -Offline`。已有可移植目录时可添加 `-SkipPortableBuild`；脚本会先核验目录内 SHA256 清单。

数据默认写入 `%APPDATA%/QuickTool-Rust`。测试可用 `QUICKTOOL_RUST_DATA_DIR` 指定隔离数据目录。原版 QuickTool 数据暂不自动导入。

显式导入旧设置与历史：`QuickTool.exe --import-from OLD_DIR --import-to NEW_DIR`。目标目录必须不存在；旧目录只读。原文件备份、坏记录及错误报告保存在新目录。退出码 `0` 表示成功，`3` 表示导入完成但有错误记录，`2` 表示失败。使用 `QuickTool.exe --data-dir NEW_DIR` 运行导入后的数据。

## 验证

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
```

`reference/` 仅用于迁移对照；`scripts/capture-baseline.cjs` 从原版提取差分用例。测试直接比较 Rust 与原版的输出，不以 Rust 自身计算结果作为预期值。

文件系统通知集成测试使用随机生成的临时文件。受限 Windows 沙箱可能禁止重命名；需在具备正常临时目录权限的环境运行。

## 性能证据

[像素基准](docs/pixel-benchmark.json)：同一输入与滤镜参数，1,048,576 像素，预热 3 次、采样 7 次。当前中位耗时 JS 88.93 ms，Rust Release 54.80 ms（约 1.62 倍），结果校验一致。此结果仅说明像素计算性能，不能代表整机启动、内存、搜索或拖拽性能。

[应用基准](docs/app-benchmark.json)：最新同机三轮隔离采样，启动中位数2.889s → 1.025s，空闲工作集约845MiB → 234MiB。启动从应用入口计时，不包括OS创建进程；原版内存不含外部辅助进程。样本有波动，不同渲染器的布局延迟不能直接等同。

[索引基准](docs/index-benchmark.json)：10031条合成文件，整数ID版两次扫描566/694ms，同日原版2234ms；Rust数据库从约79MB降至14MB，原版约11MB。测量边界、文件清单和完整样本均保留。

当前143项测试通过、5项跳过；1032条原版编辑对照与23步应用内整窗事件通过。系统级输入、多屏/DPI及像素完全一致性仍未验收。

## 面板唤起回归

alpha.33 修复Windows隐藏窗口无法处理托盘/快捷键唤起，以及唤起被同帧失焦事件取消的问题。后台通知明确唤醒主窗口。安装新版本前先退出旧Rust版本；若旧托盘无法退出，可在任务管理器结束对应进程。

执行 `./scripts/Verify-Summon.ps1` 可用临时配置复验三轮真实托盘回调；不发送系统键鼠输入。原生隐藏窗口最小回归：`cargo run --example hidden_wake -- REPORT.json`。

## 状态

见 [持续工作记录](AGENT_WORKLOG.md) 与 [迁移验收清单](docs/migration.md)。
