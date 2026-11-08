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

### macOS（M 系列）

在 Mac（或 CI 的 macOS runner）上执行 `bash scripts/Build-Rust-macOS.sh`：构建
aarch64 原生二进制，产出 `QuickTool.app`（Info.plist、icns、全部 13 模块、许可、
ad-hoc 签名）并打包 `releases/QuickTool-Rust-<version>-macos-aarch64.dmg` 与
SHA256。CI 由 `.github/workflows/build-rust-macos.yml`（macos-14）自动构建、挂载
校验并上传构件。工具、搜索、候选、模块等功能与 Windows 一致；DWM Acrylic、
注册表搜索、Everything、PATH 启动、Win32 托盘为 Windows 专属，macOS 上优雅降级
并在面板中提示。最终链接需要 macOS SDK（Apple 许可），故交叉编译只完成到类型级；
`cargo zigbuild --target aarch64-apple-darwin` 已验证全部 Rust 源与 bundled SQLite
C 依赖编译通过。与原版一致性数据见 [一致性报告](docs/parity-report-alpha39.md)。

安装包位于 `releases/QuickTool-Rust-Setup-0.6.0-alpha.41-x64.exe`，旁边提供 SHA256 校验文件。默认安装到当前用户的 `LocalAppData/Programs/QuickTool Rust`，创建快捷方式及卸载入口；卸载保留设置和历史。

使用已有 NSIS 编译器生成安装包：`./scripts/Build-Installer.ps1 -MakeNsis PATH_TO_MAKENSIS -Offline`。已有可移植目录时可添加 `-SkipPortableBuild`；脚本会先核验目录内 SHA256 清单。

## 可选模块

自 alpha.38 起，功能工具按模块拆分为可独立安装的可选组件；启动器核心（帮助、设置、历史、运行/打开/终端等）始终内置。可选模块：计算器、时间日期、颜色转换、单位换算、进制转换、JSON、文本与编码、字符参考（约 7 MB 数据）、国家地区（约 5 MB 数据）、钉贴、剪贴板、文件搜索、系统工具。

- **安装包**：NSIS 组件选择页逐个勾选模块，默认全部勾选（与原版开箱行为一致）；所有模块包随核心落盘在 `assets/packages/`。
- **设置 → 模块管理**：设置页首行为独立入口，点击进入单独的模块管理页（返回/Esc 逐层退出），随时安装或卸载模块，即时生效，无需联网；卸载不删除 `%APPDATA%` 下的钉贴、历史等用户数据，重新安装后自动恢复。
- **机制**：模块已安装等价于 `assets/modules/<id>/manifest.json` 存在；数据（Unicode 索引、国家数据、单位表、系统命令清单）从模块目录按需加载，未安装时相关命令从建议、检测与帮助中隐藏。
- **未安装回退**：与原版命令全集对齐，精确匹配未安装模块命令的查询（如 `pin`、`ping`）仍显示"模块未安装"提示行并可跳转模块管理，不会让无关的本机应用（如 PING）成为首选；部分匹配仍保持隐藏。

数据默认写入 `%APPDATA%/QuickTool-Rust`。测试可用 `QUICKTOOL_RUST_DATA_DIR` 指定隔离数据目录，`QUICKTOOL_RUST_ASSETS_DIR` 指定隔离模块目录。原版 QuickTool 数据暂不自动导入。

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

既有原版编辑对照与本次搜索差分均保留自动验证。系统级输入、多屏/DPI及像素完全一致性仍未验收。

## 面板唤起回归

alpha.33 修复Windows隐藏窗口无法处理托盘/快捷键唤起，以及唤起被同帧失焦事件取消的问题。后台通知明确唤醒主窗口。安装新版本前先退出旧Rust版本；若旧托盘无法退出，可在任务管理器结束对应进程。

执行 `./scripts/Verify-Summon.ps1` 可用临时配置复验三轮真实托盘回调；不发送系统键鼠输入。原生隐藏窗口最小回归：`cargo run --example hidden_wake -- REPORT.json`。

## 状态

见 [持续工作记录](AGENT_WORKLOG.md) 与 [迁移验收清单](docs/migration.md)。

alpha.34 修复Windows WGL错误关闭透明子窗口属性的问题，恢复Glow等子窗口的DWM透明合成。原生回归通过自有绿色背景窗口验证透出颜色，运行 `cargo run --example transparent_child -- REPORT.json` 可复验。

alpha.35 修复窗口展开后设置、候选和历史被初始圆角区域裁切的问题。运行 scripts/Verify-Expanded.ps1 可检查实际原生窗口的区域、命中及内容像素，覆盖收起后再次展开。

alpha.36 对齐主面板固定位置、唤起全选、标签/历史焦点、设置保护及固定快捷键。高度动画改为内部绘制，原生空间只在转换边界调整；Glow以60Hz调度并缓存静态几何。对照和测量边界见 docs/ux-parity-alpha36.md。

alpha.37 修复输入后图标加载改变行高、重复自动滚动及旧查询文件结果混入的问题；恢复`ch`瑞士卡片、混合候选和原版排序规则。197组实际候选对照、14组输入及9组连续输入通过。实现差异、测量与未验证边界见[搜索对照报告](docs/search-parity-alpha37.md)。

alpha.38 引入可选模块体系：13 个功能模块可独立装卸（安装包组件页 + 设置内模块管理），Unicode 索引、国家数据、单位表与系统命令清单改为磁盘按需加载，二进制体积相应减小。模块门控与安装/卸载生命周期由 `tests/modules_gating.rs` 覆盖。

alpha.41 修复窗口焦点与 IME 组合的失焦关闭：窗口可视且非设置/弹窗/菜单/录制状态时，每帧自动将焦点归还搜索框（输入始终落在输入框）；IME 组合进行中的失焦（候选窗交互）不再隐藏窗口（组合结束且仍失焦时延迟关闭），非组合失焦照常立即关闭；IME 过滤器区分组合进行中与仅 Enabled 帧（Windows Enabled=组合开始、macOS Enabled=IME 切换）， Enabled 帧的打字不再被误吞；交互探针扩至 39 项。

alpha.40 修复 IME 组合期间按键误触发命令的问题：egui-winit 原样转发组合期间的按键，Enter 提交候选词被当作执行命令；现按原版浏览器语义（组合中 key=Process）在含 IME 事件的帧内丢弃无修饰键的 Key 按下与 Text 事件（所有 viewport：主面板、大输入、Pin、选择器、菜单），Ctrl/Cmd 快捷键保留，组合结束后的真实 Enter 照常执行；交互探针新增 4 组 IME 回归（36/36）。

alpha.39 修复模块门控与原版 UX 的冲突：安装包组件页改为默认勾选全部模块（原版所有命令开箱即用）；裸输入精确命中未安装模块的命令时显示"模块未安装"提示并可跳转模块管理，不再让无关本机应用（如输入 pin/ping 时的 PING）成为首选；激活未安装模块的结果行会打开模块管理；修复 `/ping` 在系统模块缺失时结果空白的问题。候选对照扩展至 206 组（新增裸输入 pin/ping/pinw 与 PING 应用夹具的 9 组跨标签用例）。
