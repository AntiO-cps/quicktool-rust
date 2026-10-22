## 全 Rust 重写（独立目录与分支）

| 字段 | 内容 |
| --- | --- |
| 状态 | `进行中` |
| 时间 | `2026-10-20T00:07:15+08:00` → 进行中 |
| 最后更新 | `2026-10-22T23:54:53+08:00` |
| Git | 独立目录 `codex/rust-rewrite` · 无初始提交 → `e657794924e7a466759855a431fcf87fea7664d9`（最新几何校准代码；核心整合8ecdb26）；原目录 `master` 无提交 |

### 目标
在独立目录将 UI、交互与业务逻辑全部迁移至 Rust，保持原版一致，并测量性能。

### 修改
- `Cargo.toml`、`Cargo.lock`、`src/`：原生 Rust UI、工具、Windows 集成、托盘、Glow、文字/图片 Pin、标注、PNG/PSD、撤销、历史与设置；导出原子替换。
- `src/dates.rs`、`src/js_case*.rs`、`src/text.rs`、`src/search.rs`：固定原版时区2025b及Unicode15.1行为；历史/未来夏令时、JS日期端点、UTF16大小写及模糊搜索对齐。
- `src/index.rs`：整数ID倒排、稳定ID旧库原子迁移、频率缓存与查询/扫描优化；压缩后ID保持有效。
- `src/large_input.rs`、`src/image_decode.rs`、`src/pickers.rs`、`src/interaction_probe.rs`：统一编辑器、4096字节布局窗口、后台图片解码、独立日期/颜色窗口；主窗口隐藏时关闭拥有的弹窗。
- `src/history*.rs`、`src/storage.rs`、`src/pin_storage.rs`、`src/import_legacy.rs`：原编码JPEG/WebP/GIF历史、资产去重/损坏保护/回滚、旧名称保留、Pin历史图处理及整行恢复。
- `src/screen_picker.rs`、`src/eyedrop_overlay.rs`：冻结桌面取色、预览及覆盖窗口；系统输入仍待验收。
- `assets/`、`vendor/epaint/`、`vendor/eframe/`：原字体/图标/旗帜与许可、字体轴及独立窗口截图/编辑命令补丁；只读Unicode二进制索引。
- `tests/`、`reference/`、`scripts/`：原Electron差分夹具、事件/性能探针、打包与原版哈希校验。
- `docs/`、`README.md`：验收状态、测量边界、原始性能样本与复现说明。
- `.gitattributes`、`.gitignore`：换行规范及构建缓存、测试数据、发布目录排除；两处 `AGENT_WORKLOG.md` 保持同一任务条目。
- 本地 Git 身份按用户提供设为 codex <codex@example.com>；未改全局配置。

### 验证
- `PASS` `cargo test --offline --all-targets --target-dir .../quicktool-system-target`：142项通过、5项跳过，完整日志 `.qa/full-tests.log`。包括566计算、430数字、143668 Unicode名称/类别、120 Unicode、550跨11时区日期、456模糊搜索、21组大小写、48滤镜及历史图、2860几何与PSD/Canvas对照。
- `PASS` 索引5项回归：553条/55组原Electron查询、实扫、通知、重命名、持久化、旧库迁移及压缩后ID有效；性能基准按需跳过。
- `PASS` 原版编辑对照：192 Ctrl + 840普通输入用例；JPEG/WebP/GIF原编码等6项历史资产回归；历史面板2项真实egui点击测试。
- `PASS` 生命周期最终修复后：27项UI单元测试再通过；23步实际eframe帧事件探针通过，报告 `docs/ui-interaction-probe.json`。
- `PASS` `cargo clippy --offline --all-targets -- -D warnings`、`cargo fmt --all -- --check`：最终代码通过。
- `PASS` `cargo build --offline --release --bin quicktool-rust`：最终发布构建成功，64513536字节。
- `PASS` `scripts/Build-Rust.ps1 -Offline -SkipBuild`：更新可移植程序、资源、文档及依赖许可索引；971个文件SHA256逐一匹配。许可索引277条，部分上游crate未附独立许可文件，索引保留声明，不视为完整许可审计。
- `PASS` 最终打包程序隔离运行：0xFF候选截图生成，正常退出0；独立日期/颜色子窗口及历史截图已检查，仍存在已知视觉差异。
- `PASS` 三轮应用采样：启动中位2.889s→1.025s、空闲工作集约845MiB→234MiB。万条索引两次566/694ms，原版2234ms；数据库13606912字节。测量边界及样本见 `docs/`，不代表所有机器同比提升。
- `PASS` 16MB有效PNG DataURL：debug至候选/GPU截图3.84秒，峰值working set416358400字节；不代表系统输入呈现延迟。
- `PASS` `node scripts/verify-original.cjs`：589/589原版源文件未改变。
- `PASS` 原有隔离OS输入测试进程：提权CIM按PID核验确认已不存在。
- `SKIPPED` 5项：子进程sleep夹具、手动时区性能基准、手动万条索引性能基准、Windows Search服务测试、私有window station剪贴板测试；未操作真实用户剪贴板。
- `BLOCKED` 系统级鼠标/键盘全流程：computer-use窗口访问授权超时，未发送OS输入；应用内事件与截图不代替此项。

- `PASS` 暂存diff与敏感信息模式检查：无空白错误、未发现匹配；只提交本任务源码/夹具/文档，构建与隔离数据未入库。

- `PASS` 后续几何修复：历史首卡在125%DPI下与原版同为x22..992/y147..230；主框固定边界、透明滚动轨道已实际截图复核。最终27项UI测试、Clippy、fmt、Release、打包程序隔离截图及23步探针全部再通过；对照记录 `docs/visual-geometry.json`。

### 结论
- 已完成：独立目录/分支、上述原生Rust实现、差分/回归、性能采样及当前可移植阶段版本。
- 未完成：UI与交互完全一致；字体/图标抗锯齿及少量边界像素差异仍存在，需继续视觉校准。
- 未验证：系统级键鼠/IME、多屏/DPI、实际滴管/拖拽、Everything与Windows Search非空结果、真实管理员/终端/Regedit启动及用户剪贴板；完整依赖许可审计。
- 风险：整体目标仍在进行中，不能将阶段版本视为已满足完全一致的替代版本。
