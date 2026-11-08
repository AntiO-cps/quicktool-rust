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

## 生成 Rust Windows 安装包

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-22T23:58:34+08:00` → `2026-10-23T00:05:10+08:00` |
| 最后更新 | `2026-10-23T00:05:30+08:00` |
| Git | `codex/rust-rewrite` · `ef0b9ee` → `84d206c23110d27a909a83917ce2eb30e4ff2b84` |

### 目标
提供可双击安装的 Windows x64 安装包。

### 修改
- `scripts/installer.nsi`：当前用户安装、中文/英文安装向导、目录选择、快捷方式及卸载入口；默认不启动应用，卸载按安装文件清单删除，保留额外文件及用户数据。
- `scripts/Build-Installer.ps1`：检查便携目录SHA256、生成精确卸载清单、编译NSIS安装包并输出SHA256校验文件。
- `README.md`、`.gitignore`：安装/构建说明及生成目录排除。
- `docs/installer-verification.json`：实际安装包摘要、大小及隔离安装/卸载验证结果。
- `releases/QuickTool-Rust-Setup-0.6.0-alpha.32-x64.exe`及`.sha256`：本地安装包21625429字节；二进制按仓库规则忽略，不进入Git。
- 两处 `AGENT_WORKLOG.md`：本任务单条记录。

### 验证
- `PASS` 修改前Git检查：工作区干净，起点ef0b9ee；仅修改本任务文件。
- `PASS` NSIS3.0.4.1工具：使用原项目依赖中固定的下载URL与SHA512校验，仅置于任务缓存；未全局安装。
- `FAILED` 受限沙箱启动makensis：DLL重定位错误；同一已校验工具在正常Windows进程中编译成功。
- `PASS` `scripts/Build-Installer.ps1 -SkipPortableBuild -Offline -MakeNsis ...`：生成安装包与SHA256；包SHA256为1FEA7ED23D28CEAAF3D0A4995C7272EC5075341DE6E19E0F8CB2376BC0EB97D1。
- `PASS` 两次隔离中文及空格路径静默安装：退出0；971项payload哈希一致；安装后的主程序摘要EAAE511083C005CA842695B2F2B030867229BF05C9C7BA984D62FCB7E3657DC9。
- `FAILED` 首次卸载验证脚本：只等主程序删除，过早检查尚在执行的卸载子进程；实际随后卸载完毕，目录仅剩两个测试数据文件。
- `PASS` 修正等待全部payload消失后的隔离卸载回归：安装文件全部删除，额外sentinel及测试设置保留。使用/NOINTEGRATION，未创建正式快捷方式或注册表卸载项。
- `PASS` `git diff --check`：通过；脚本已实际编译执行。
- `PASS` `Get-AuthenticodeSignature`：确认当前安装包未签名。
- `NOT_RUN` 安装向导鼠标操作、正式快捷方式及注册表集成：本次采用禁用系统集成的隔离静默验收。

### 结论
- 已完成：安装包、摘要文件、可复现构建脚本及隔离安装/卸载验证。
- 未完成：无（本任务仅打包，整体Rust一致性目标仍按前条记录继续）。
- 未验证：安装向导手工流程与正式系统集成。
- 风险：未签名安装包；沿用当前Rust阶段版本的已知一致性限制。

## 修复 Rust 面板无法唤起

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-23T00:08:18+08:00` → `2026-10-23T00:41:14+08:00` |
| 最后更新 | `2026-10-23T00:41:55+08:00` |
| Git | `codex/rust-rewrite` · `b94bc0a` → `170bad913f5f67f6036629f3c3097f929dfc2d33` |

### 目标
定位并修复安装版面板无法唤起，交付已验证的修复安装包。

### 修改
- `vendor/eframe/src/native/run.rs`：Windows隐藏窗口由事件循环线程执行到期唤醒帧，避免等待不会送达的WM_PAINT，托盘/快捷键事件可被消费。
- `src/ui.rs`、`src/history_panel.rs`：后台回调明确唤醒ROOT；统一显示、焦点、最小化恢复；清除旧失焦状态，避免同帧取消明确唤起。
- `src/platform.rs`：增加快捷键线程注册、原生消息回调、唤醒及注销的回归。
- `examples/hidden_wake.rs`：独立原生隐藏窗口回归；只使用测试窗口及后台通道。
- `src/summon_probe.rs`、`scripts/Verify-Summon.ps1`：临时配置下连续三轮完整程序原生托盘回调验收；验证脚本显式释放进程句柄，避免影响紧随其后的卸载测试。
- `Cargo.toml`、`Cargo.lock`：版本升为0.6.0-alpha.33。
- `README.md`、`docs/migration.md`、`docs/panel-wake-verification.json`、`docs/installer-alpha33-verification.json`：根因、复现、构建及安装验证证据。
- `releases/QuickTool-Rust-Setup-0.6.0-alpha.33-x64.exe`及`.sha256`：修复安装包21608760字节；二进制忽略，不纳入Git。
- 两处 `AGENT_WORKLOG.md`：本任务单条持续记录。

### 验证
- `PASS` 修改前Git检查：工作区干净，HEAD b94bc0a。
- `FAILED` 修复前独立原生隐藏窗口回归：hiddenBeforeWake=true、visibleAfterWake=false，复现托盘唤起事件无法消费的底层条件。
- `PASS` 同一原生回归修复后：hiddenBeforeWake=true、visibleAfterWake=true。
- `PASS` 原生快捷键线程回归：注册成功；只向本进程线程投递WM_HOTKEY，事件与唤醒到达；Drop后同一键可重新注册，未发送OS键盘输入。
- `PASS` 最终源码 `cargo test --offline --all-targets`：143项通过、5项按需/环境检查跳过；最终日志 `.qa/panel-wake-full-tests.log`。
- `PASS` `cargo clippy --offline --all-targets -- -D warnings`、`cargo fmt --all -- --check`：最终源码通过。
- `PASS` 原有完整应用23步事件回归；编辑/菜单/日期/颜色逻辑保持通过。
- `PASS` debug、发布态以及最终实际安装目录中的程序：各连续三轮托盘回调恢复主窗口并保持可见，开启hideOnBlur；仅向自身托盘通知窗口投递回调，无OS键鼠输入。
- `PASS` 最终发布重新构建、便携目录与NSIS安装包生成；最终安装程序的主EXE与最终构建SHA256完全一致：3A0ACF019CB9479BB8031FAD953EC10F9A28268EDDB464021236511501587F1D。
- `PASS` 修复安装包SHA256：108FAF41EA5F07B455607F017AA0CDBD45C326C19A6A87A751B654D590A6F351；973项已安装payload逐项校验。
- `FAILED` 一次卸载自检残留主EXE：PowerShell测试脚本未及时释放已退出程序的进程句柄；脚本结束后EXE可独占打开。修正Dispose后重新安装/运行/卸载通过。
- `PASS` 最终隔离卸载：安装文件全部删除，额外sentinel保留；正式快捷方式/注册表集成禁用，用户设置、历史及其他进程未修改。
- `PASS` 原版源码589/589哈希未变；任务diff空白及敏感信息模式扫描通过。
- `PASS` 只读进程检查：发现同一正式Rust安装目录有两个旧进程，未终止它们；更新说明要求退出旧版，避免快捷键占用。
- `NOT_RUN` 物理鼠标/键盘、正式快捷方式和注册表集成：使用自身原生回调及隔离安装验收，不冒充手工操作。

### 结论
- 已完成：根因复现、隐藏窗口唤醒及焦点竞态修复、回归、修复版安装包及安装后的托盘唤起复验。
- 未完成：无（此项修复不代表整体Rust UI一致性目标已完成）。
- 未验证：物理键鼠手工流程、正式系统集成；迁移阶段其他已知差异沿用前述记录。
- 风险：用户的两个旧版进程仍在运行，需退出后安装新版；安装包未签名。

## 修复 Rust 面板及 Glow 黑底

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-23T01:15:33+08:00` → `2026-10-23T01:28:15+08:00` |
| 最后更新 | `2026-10-23T01:28:27+08:00` |
| Git | `codex/rust-rewrite` · `d1d670e` → `d6544a2d6ed045e1815633b76db9b6a2951bfe97` |

### 目标
修复原生透明合成，使主面板和外发光背景正确透出桌面，并更新安装包。

### 修改
- `vendor/eframe/src/native/glow_integration.rs`：Windows子窗口创建保留请求的透明属性；不让glutin依据WGL TRANSPARENT_ARB标志关闭DWM透明。其他平台沿用原finalize_window路径。
- `examples/transparent_child.rs`：自有绿色host与透明child原生合成像素回归；透明区域应透出绿色，不透明红色内容应保留；包含Glow的鼠标穿透/非激活/无任务栏/置顶属性和主面板DWM材质/圆角配置。
- `Cargo.toml`、`Cargo.lock`：升为0.6.0-alpha.34。
- `README.md`、`docs/migration.md`、`docs/transparency-verification.json`、`docs/installer-alpha34-verification.json`：修复说明、实际前后颜色及安装验证证据。
- `releases/QuickTool-Rust-Setup-0.6.0-alpha.34-x64.exe`及`.sha256`：本地修复包21617113字节；按仓库规则忽略二进制。
- 两处 `AGENT_WORKLOG.md`：本任务单条记录；未调整用户颜色、透明度或主题设置。

### 验证
- `PASS` 修改前Git检查：工作区干净，HEAD d1d670e。
- `PASS` 附图检查：1415×505 RGB，外部纯黑，尺寸与Glow窗口一致。
- `FAILED` 修复前 `cargo run --offline --example transparent_child -- ...`：透明区域期望绿色[40,180,90]，原生实际采样为黑色[0,0,0]，复现问题。
- `PASS` 同一基础回归修复后：透明区域原生实际[40,180,90]与背景精确一致。
- `PASS` 扩展原生回归：采用Glow鼠标穿透/非激活等属性及主面板DWM样式后，透明区域仍为[40,180,90]，不透明内容为[255,0,0]；仅采样自身测试窗口覆盖范围，未保存其他桌面内容或发送输入。
- `PASS` `cargo test --offline --all-targets`：143项通过、5项按需/环境检查跳过；日志`.qa/transparency-full-tests.log`。
- `PASS` `cargo clippy --offline --all-targets -- -D warnings`、`cargo fmt --all -- --check`：通过。
- `PASS` `cargo build --offline --release --bin quicktool-rust`、便携目录及NSIS构建：alpha.34生成成功。
- `PASS` 最终实际隔离安装：975项payload哈希一致，安装后主EXE与最终发布构建一致（8A224976DC6EB49EB1F2415FB95B2EB1425B074C357A01DB38B6B34E7CD282BC）。
- `PASS` 安装后的程序连续三轮原生托盘回调唤起、保持可见；上轮隐藏窗口修复保持通过。
- `PASS` 最终隔离卸载：安装文件全部删除，额外sentinel保留；正式快捷方式/注册表集成禁用，用户设置、历史和运行进程未修改。
- `PASS` 安装包SHA256：BEBD7DE645BBBA8DAB42709B4B1705AF67AE89B860E803A3C195D2F7D7CF1B60。
- `PASS` 原版源码589/589哈希保持一致；diff空白检查通过。
- `NOT_RUN` 所有GPU/驱动组合及完整物理键鼠验收：原生实测覆盖当前Windows环境，其他迁移限制沿用前项记录。

### 结论
- 已完成：黑底根因复现、透明窗口创建修复、实际DWM合成验证、回归及alpha.34安装包。
- 未完成：无（不代表整体Rust UI完全一致目标已完成）。
- 未验证：其他GPU/驱动环境、物理键鼠及正式系统集成的完整人工流程。
- 风险：安装包未签名；更新前需退出旧Rust版本。

## 修复展开面板内容被原生区域裁切

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-23T01:39:28+08:00` → `2026-10-23T01:49:10+08:00` |
| 最后更新 | `2026-10-23T01:49:20+08:00` |
| Git | `codex/rust-rewrite` · `ef32b05` → `357b416acebe6cd6bd4a620eb22822c7138981ec` |

### 目标
修复设置和候选列表展开后内容不可见，验证正常原生样式路径并更新安装包。

### 修改
- `src/ui.rs`：样式缓存包含实际原生客户区尺寸；原生样式应用成功后记录缓存。
- `src/expanded_probe.rs`、`scripts/Verify-Expanded.ps1`：新增正常原生样式下6种展开/收起回归，隔离用户数据。
- `Cargo.toml`、`Cargo.lock`、`README.md`：更新alpha.35版本和回归说明。
- `docs/expanded-window-verification.json`、`docs/installer-alpha35-verification.json`：记录修复前后原生证据及安装包验证。

### 验证
- `PASS` 修改前Git检查：工作区干净，HEAD ef32b05。
- `PASS` 原生复现：窗口1015×803，旧区域1015×105，5个展开状态失败；修复后6/6状态通过。
- `PASS` `cargo test --offline --all-targets`：正常窗口站全部通过，5项按既有条件忽略。
- `BLOCKED` 沙箱内初次测试：Windows拒绝快捷键注册；同一测试在正常窗口站通过。
- `PASS` Clippy严格检查、fmt检查、原项目589/589文件哈希检查。
- `PASS` `--verify-ui`：23项应用内部交互回归通过。

- `PASS` release构建与NSIS打包：alpha.35，21648059字节，SHA256 13EF5F9506D20BB3025CF4C4EB7888023C7BF734D504F728B1FBF9A16F17DF8F。
- `PASS` 安装后文件清单哈希与最终release二进制匹配；6/6原生展开及3轮托盘唤起通过；卸载清理包文件且保留无关用户文件。
- `PASS` 自有绿色host/透明child原生透明合成fixture复验。
- `PASS` Git diff空白检查及本任务新增代码/报告敏感字段扫描。

### 结论
- 已完成：原生窗口裁切修复、展开/交互回归及alpha.35安装包。
- 未完成：无（本次缺陷修复范围）；整体Rust迁移验收仍在进行中。
- 未验证：人工鼠标键盘验收、多显示器DPI切换；迁移整体完全一致性。
- 风险：沿用迁移阶段其他已知差异。

## 对齐固定窗口UX并优化动画性能

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-23T23:40:47+08:00` → `2026-10-24T00:05:31+08:00` |
| 最后更新 | `2026-10-24T00:06:12+08:00` |
| Git | `codex/rust-rewrite` · `bcc6987` → `7902407724e9fcd31f5200199d88ae8733f15e84` |

### 目标
对照原项目固定窗口及常用交互，修正Rust版差异并测量优化动画性能，提供新安装包。

### 修改
- `src/launcher_native.rs`、`src/main.rs`：固定工作区22%锚点、按鼠标显示器唤起与DPI/可用空间计算、阻止主面板移动、首帧定位后显示。
- `src/launcher_motion.rs`、`src/ui.rs`：预分配原生空间、180ms内部高度动画与210ms收尾；唤起全选、历史/设置保护、候选标签/焦点和失焦策略对齐。
- `src/theme.rs`：60Hz调度、缓存角度与索引、按状态重配原生Glow，分离视觉/分配尺寸。
- `src/backdrop.rs`：内容高度动画只更新合成层、缓存原生尺寸/材质更新，保留Pin接口。
- `src/interaction_probe.rs`、`src/motion_probe.rs`：新增使用习惯回归与真实动画开启时的帧/尺寸测量。
- `Cargo.toml`、`Cargo.lock`、`src/lib.rs`、`README.md`：集成模块和HiDpi，版本更新alpha.36。
- `docs/animation-verification.json`、`docs/installer-alpha36-verification.json`、`docs/ux-parity-alpha36.md`：源规则对照、原始测量与边界。

### 验证
- `PASS` 修改前Git检查：工作区干净，分支codex/rust-rewrite，HEAD bcc6987。

- `PASS` 安装后29项应用帧交互（含失焦策略）、6/6原生展开及3轮托盘唤起；原生窗口定位/禁移测试通过。
- `PASS` 动画开启debug同机8次切换：真实窗口resize每次3–5→1，root帧间隔P95约23–85ms→8–15ms；非OS呈现FPS，基线含共享缓存优化。
- `PASS` 正常原生样式展开6/6；Glow几何3项、Backdrop策略3项、布局3项定向通过。
- `FAILED` 初次全目标高并发编译：系统页面文件容量不足；改为-j 2。
- `FAILED` 原生测试初次启动：GetWindowSubclass静态导入在测试进程缺入口；去除该导入，用自有wndproc行为验证；定向测试重跑通过。

- `PASS` 最终全目标测试：全部通过，5项既有条件忽略；严格Clippy及fmt通过；原版589/589哈希不变。
- `FAILED` 初次严格Clippy：测试模块后含Drop实现；移动模块顺序后重跑通过。

- `PASS` release构建及完整设置页渲染图核对；alpha.36安装包21615617字节，SHA256 9432849B576874428C4D65A7DA0ED297407B06A9A796F502D49775EF9993E319。
- `PASS` 中文隔离路径安装：980个payload文件哈希匹配，安装二进制与最终release一致；卸载清理payload并保留无关文件，正式注册集成在隔离测试禁用。
- `PASS` 安装后8次真实动画：每次原生resize 1次，root帧间隔P95约9–14ms；不等同OS呈现FPS。
- `PASS` diff空白及新增代码/报告敏感字段检查。

### 结论
- 已完成：固定主面板及习惯对齐、动画优化、完整自动回归、alpha.36安装包及安装后复验。
- 未完成：无（本次UX修正与动画优化范围）；整体Rust迁移仍在进行中。
- 未验证：实际系统输入、多屏DPI及完整UX一致性。
- 风险：系统物理输入、多显示器/GPU与整体像素一致性尚未完成人工验收。

## 彻查输入抖动与搜索实现差异

| 字段 | 内容 |
| --- | --- |
| 状态 | `已完成` |
| 时间 | `2026-10-24T01:20:25+08:00` → `2026-10-24T02:15:38+08:00` |
| 最后更新 | `2026-10-24T02:16:17+08:00` |
| Git | `codex/rust-rewrite` · `ac0bcab` → `b9c3b48d5880b8cade5080602ead825d1f3d0865` |

### 目标
复现搜索输入抖动、恢复ch瑞士等原版候选，并通过完整管线差分审计修正实现差异。

### 修改
- `src/ui.rs`：统一混合候选、国家元数据/备用项行动、全量加权后截断、日期compact截断、模式分值、应用/文件/注册表展示与行动分离、JS空白解析、查询代际和滚动状态。
- `src/country_card.rs`、`src/host_candidates.rs`：按每行候选绘制国家卡及构建宿主候选；图标占位固定34px避免异步图标改变62px行高。
- `src/countries.rs`、`src/tools.rs`、`src/colors.rs`：国家匹配/补全、子命令模糊匹配、短色变体摘要和JS toFixed取整。
- `src/storage.rs`、`tests/storage.rs`、`Cargo.toml`：usage改为保留JSON插入顺序的IndexMap，维持同时间最近列表稳定顺序；alpha.37版本。
- `src/candidates_probe.rs`、`src/search_layout_probe.rs`、`src/interaction_probe.rs`：实际App候选差分、真实编辑事件逐帧布局和国家交互回归。
- `scripts/capture-*.cjs`、`scripts/export-candidates-oracle.cjs`及新增oracle/tests：直接执行原JS生成固定数据作完整对照，原源码不修改。
- `README.md`、`docs/*alpha37*`：记录实现差异、原版来源、候选/布局证据与安装验证边界。

### 验证
- `PASS` 修改前Git检查：工作区干净，HEAD ac0bcab。

- `PASS` 国家8956查询/249国家、命令1035查询/14分隔/827子模式、169颜色/8检测行、存储7项定向回归。
- `PASS` 逐帧定位：同样4个应用候选在图标到达时行高62→66、窗口多次扩容；固定图标槽后行高始终62。
- `PASS` 最终debug真实编辑事件：14组普通输入、9组连续输入及32项交互通过；静止阶段目标/滚动/选中行变化均0、行高稳定、窗口锚点不变。
- `PASS` 实际候选差分197/197通过，0失败、0未验证；包含随机语义验证及剪贴板缓存展示。
- `PASS` cargo test --offline -j 2 --all-targets：正常桌面权限下165通过、5项既有环境测试跳过；Clippy -D warnings、fmt与589/589原版源码校验通过。
- `FAILED` 受限沙箱首次完整测试：隔离快捷键注册被拒绝访问；正常桌面权限重跑通过。初次严格Clippy报告测试排序写法，改用sort_by_key后通过。
- `PASS` Release构建；截图确认ch瑞士卡片及其他工具混合显示。
- `PASS` alpha.37安装至独立中文目录：984个载荷文件逐项SHA256校验、EXE与最终release一致；实际安装后6项展开、3轮托盘、8项动画、32项交互、197组候选、14+9组输入全部通过；卸载清除载荷并保留无关文件。
- `PASS` git diff --check；本任务新增/修改源码和报告敏感凭据模式扫描未发现命中。

### 结论
- 已完成：输入抖动及搜索差异修复、原版差分、完整自动回归、alpha.37安装包及安装后复验。
- 未完成：无（本次搜索修复范围）；整体Rust迁移仍在进行中。
- 未验证：完整物理输入、多屏DPI及全部功能的人工一致性。
- 风险：物理输入、多显示器/驱动、真实剪贴板及搜索提供方环境仍有验收边界。

## alpha.38 — 功能模块化：可选安装模块体系

### 目标
将全部功能工具从单一二进制拆为可独立安装的可选模块：安装包组件选择页逐项勾选，设置面板随时安装/卸载，离线可用。

### 修改
- `assets/module-catalog.json`、`assets/modules/<id>/manifest.json`：13个可选模块（计算器、时间日期、颜色转换、单位换算、进制转换、JSON、文本与编码、字符参考、国家地区、钉贴、剪贴板、文件搜索、系统工具）+ 10个核心命令的目录与清单；模块已安装等价于 `assets/modules/<id>/manifest.json` 存在。
- `src/modules.rs`：新增模块注册/路径解析（exe目录优先、`QUICKTOOL_RUST_ASSETS_DIR` 测试隔离）、`LeakedCache` 版本化数据缓存（模块集变化自动失效）、install/uninstall（复制自 `assets/packages/<id>`、卸载保留用户数据）、`is_enabled` 门控（首次 refresh 前全量可用以兼容既有测试）。
- `src/tools.rs`：建议与 help 按 installed 过滤、execute 前置"模块未安装"守卫、system-commands/windows-settings 从模块目录磁盘加载（不再内嵌）。
- `src/unicode.rs`、`src/countries.rs`、`src/units.rs`：6.9MB Unicode 索引、国家数据、单位表去内嵌改为按代际泄漏的磁盘懒加载；Unicode 缺失时 ASCII 类别降级保持文字编辑行为；旗帜路径改走模块目录（`country_card.rs`、`system.rs`）。
- `src/ui.rs`：启动时 `modules::refresh()`；空态最近/默认行、`/`建议、检测、help、进入模式、files/registry 分支、全局文件搜索全部按模块门控；设置面板顶部新增"模块管理"区（13模块状态、体积、命令数、即时安装/卸载按钮，卸载先退出活动模式）。
- `scripts/installer.nsi`：新增 MUI 组件选择页，核心 Section（RO）+ 13 模块 Section 收进 `SectionGroup /e`（组节点全选/全不选），选中时 `CreateDirectory`+`CopyFiles` 从 packages 复制到 modules；卸载增加 `RMDir /r assets\modules`；完成页运行勾选默认开启；UTF-8 BOM。
- `scripts/Build-Rust.ps1`：staging 改为生成 `dist/assets/packages/<id>/`（不再单独复制旗帜）；输出目录先清空重建防陈旧文件泄漏。
- `tests/modules_gating.rs`：隔离包存储下门控、安装、卸载、数据缓存失效端到端回归。
- `README.md`、`CHANGELOG.md`、`Cargo.toml`：可选模块机制说明与 alpha.38 版本。

### 验证
- `PASS` `cargo test -j 2 --all-targets`：32套件全部通过（含新增模块目录完整性、manifest一致性、门控生命周期测试）；发现并修复 LeakedCache 初始版本号导致首次加载永远跳过的缺陷（大输入编辑oracle回归捕获）。
- `PASS` `cargo clippy -j 2 --all-targets -- -D warnings`、`cargo fmt --check`。
- `PASS` Release 构建：QuickTool.exe 65MB → 58.1MB（Unicode 索引去内嵌）。
- `PASS` `Build-Installer.ps1`：产出 alpha.38 安装包 + SHA256 sidecar（50B4A502AD153320195092B8A130DD05CE78EA737153C7F2C81868C7BF04E6CB）；payload 核验通过。
- `PASS` NSIS CopyFiles 递归复制语义独立实测：manifest + 249 svg + 249 png250 完整复制。
- `PASS` 隔离静默安装（/S /NOINTEGRATION）：exit 0、1006 文件、exe 与版本号正常、13 包落盘、`assets/modules` 不存在（仅核心默认勾选符合设计）。
- `NOT_RUN` 图形化组件页人工勾选交互、全选树节点点击（静默路径已覆盖布局与复制语义）。

### 结论
- 已完成：模块化拆分、安装包组件树、设置内模块管理、数据去内嵌与自动回归、alpha.38 安装包及隔离安装复验。
- 未完成：无（本次模块化范围）；整体Rust迁移仍在进行中。
- 未验证：图形组件页人工交互、卸载流程复验（沿用既有卸载清单 + 新增 modules 目录清除）。
- 风险：模块数据按代际泄漏，反复装卸大模块会累积少量内存；安装后手动删除 packages 目录将无法离线重装对应模块。

## alpha.39 — 修复模块门控与原版UX的冲突（pin→PING回归）

### 目标
排查Rust版与原版Electron的UX/操作差异：默认安装下裸输入 `pin`/`ping` 时命令整体消失，PATH索引中的PING应用成为首选并直接进入运行应用行为。对照原版候选管线后修复门控一致性，恢复原版开箱体验。

### 根因
- alpha.38 安装器模块分区默认不勾选（`Section /o`），默认安装仅核心；`tools::suggestions_all` 等建议路径按 `modules::is_enabled` 过滤，`pin`（钉贴模块）与 `system:ping`（系统工具模块）从候选中消失。
- 裸输入落入应用匹配分支：`host_candidates::apps` 对PATH应用 PING.EXE 给出 0.86 分（类别"运行"），Enter 即启动 ping.exe；原版在同等输入下首选为命令行（0.94）。
- 附带缺陷：`/ping` 在系统工具模块缺失时 `system_rows` 返回 false 但调用方仍 return，结果列表完全空白。

### 修改
- `scripts/installer.nsi`：13个模块分区去掉 `/o`，默认全部勾选（与原版所有命令开箱即用一致），用户仍可逐项取消。
- `src/tools.rs`：新增 `disabled_exact`——id或别名精确命中且模块未安装的命令，供调用方显示安装提示，避免精确命令查询落入无关应用匹配。
- `src/ui.rs`：按原版 styles.css 忠实重做设置页设计系统——panel-head（34×34 圆角图标盒、13.5/10.5 标题节奏、66px 头部与全宽底线）、setting-row（62px 行高、10px 内距、rgba .055 分隔线）、快捷键录制器（220×40 场框 + accent 键盘图标 + 等宽字）、accent 复选框（16×16）、圆角 DragValue 场框、mono 文本域场框、soft/primary 按钮（34px 高、accent 底色主按钮）、页脚底线与按钮布局；深浅两主题均按 CSS 变量取色。
- `src/ui.rs`：设置"模块管理"改为独立展开页并沿用同一设计语言（设置首行"管理" soft 入口、子页 panel-head + 模块行状态色 + soft 安装/卸载、页头返回与 Esc 逐层退出、激活未安装模块结果行直达该页、`--panel modules` 验证入口）。
- `src/ui.rs`：裸输入建议路径插入"模块未安装"提示行（分数=原精确命令行 0.94，保持与原版排序一致）；`activate` 对未安装模块的结果行统一打开设置→模块管理并提示（仅限非模式上下文）；`/ping` 无参数且系统命令数据缺失时显示模块提示行而非空白。
- `scripts/export-candidates-oracle.cjs`：国家数据路径改为模块化后的 `assets/modules/country/countries.json`；新增裸输入 `pin`/`ping`/`pinw` 携带 PING 应用夹具的 9 组跨标签用例（oracle 197→206组）。
- `tests/modules_gating.rs`：补充 `disabled_exact` 精确/部分/核心/空输入断言及安装后回退断言。
- `README.md`、`CHANGELOG.md`、`Cargo.toml`：alpha.39 版本与默认全选、未安装回退说明。
- `docs/candidates-alpha39-verification.json`：206组实际候选对照报告（全通过）。

### 验证
- `PASS` 修改前确认：原版 oracle 中裸输入 "pin" 的可视行为为 [钉住主窗口/命令 /pinapp, Ping 网络测试, PING 应用, 钉住内容]；Rust 默认安装（隔离模块目录模拟）修复前首选为应用行。
- `PASS` `cargo test -j 2 --all-targets`：168通过、5项既有环境忽略、0失败。
- `PASS` `cargo clippy -j 2 --all-targets -- -D warnings`、`cargo fmt --check`。
- `PASS` `--verify-candidates`：206/206组实际管线与原版oracle一致（含新增9组pin/ping/PING夹具）。
- `PASS` `--verify-ui`：32项应用内部交互回归通过。
- `PASS` `Verify-Expanded.ps1`：原生展开/收起回归通过。
- `PASS` 设置主页与模块管理子页渲染截图核对（`--panel settings/modules --screenshot`），与原版 .qa/original-settings.png 逐项对照（图标盒、行节奏、控件样式、页脚）；浅色主题复验。
- `PASS` 隔离模块目录（无 modules）端到端复现：裸 "ping" 实际行为为首行"系统工具 模块未安装"提示（0.94）+ PING 应用（0.86），裸 "pin" 保留钉贴提示行于原版分数位，部分查询 "pinw" 保持隐藏。
- `PASS` Release 构建与 NSIS 重新打包：alpha.39，21753941 字节，SHA256 A2AC389F77C7DB7E9DB769C0E873A7C14BAFE51E2B10CEDF27E4136C74D072A8（含设置页设计重做）。
- `PASS` 隔离静默安装（/S /NOINTEGRATION）：exit 0、13 个模块全部默认安装；安装后 QuickTool.exe 与 release 二进制哈希一致。
- `PASS` 安装后成品二进制：206/206 候选对照通过；`--panel settings/modules` 截图确认忠实还原的设置设计与模块子页。
- `PASS` 安装后成品二进制：206/206 候选对照通过；`--panel modules` 截图确认模块管理独立展开页渲染正常。
- `PASS` 隔离静默卸载：exit 0，安装目录清空。
- `NOT_RUN` NSIS 图形组件页人工勾选交互（静默路径已覆盖分区语义与文件落盘）。

### 结论
- 已完成：模块默认安装状态、未安装命令精确匹配回退、激活引导、`/ping` 空白修复、候选差分扩展与全部自动回归。
- 未完成：整体Rust迁移仍在进行中；后续出包需重新运行 Build-Installer.ps1（版本号已同步 alpha.39）。
- 未验证：物理输入与图形组件页人工交互（沿用既有边界）。
- 风险：已安装 alpha.38 默认核心的用户升级时，组件页默认全选会补装全部模块（符合本次修复意图）；卸载过的模块升级后会被重新勾选安装。

## alpha.39 — 一致性量化报告与 macOS（M 系列）交付

### 目标
继续对照原版直至可量化 ≥99% 相似度；构建实际可用的 Windows 与 macOS 安装包。

### 修改
- `src/platform.rs`：非 Windows 平台补 `save_file_dialog`（rfd 原生保存对话框，仅非 Windows 目标编译，Windows 二进制不受影响）。
- `src/backdrop.rs`：新增非 Windows `Backdrop` 占位实现（update/update_clipped 返回成功，保持调用方流程）。
- `Cargo.toml`：新增 `[target.'cfg(not(windows))'.dependencies] rfd = "0.15"`。
- `assets/assets/quicktool.icns`：由 quicktool.ico 的 PNG 条目（ic07 128 / ic08 256）生成。
- `scripts/Build-Rust-macOS.sh`、`scripts/macos/Info.plist.in`：macOS 原生构建 + QuickTool.app（全 13 模块、许可、docs、ad-hoc 签名）+ DMG + SHA256。
- `../.github/workflows/build-rust-macos.yml`：macos-14 runner 上测试（lib）、构建 DMG、挂载校验模块/图标/签名并上传构件。
- `docs/parity-report-alpha39.md`：一致性指标、22 个 oracle 套件 22,764 条用例全部通过的量化结果、运行时探针与剩余差异边界。
- `README.md`、`../CHANGELOG.md`：macOS 交付与一致性报告说明。

### 验证
- `PASS` `cargo zigbuild --target aarch64-apple-darwin`（zig 0.13 + cargo-zigbuild 0.24）：全部依赖（含 bundled SQLite C 交叉编译至 arm64-macos 目标文件）与全部 Rust 源类型编译通过；最终链接需要 macOS SDK（Apple 许可仅随 macOS 提供），由真实 Mac 或 CI runner 完成。
- `PASS` Windows 侧回归不受影响：`cargo check --all-targets`、`cargo test -j 2 --all-targets` 168 通过 0 失败、clippy/fmt 此前已过。
- `PASS` 一致性量化：22,764/22,764 差分用例（22 套件）+ 206/206 候选 + 32/32 交互 + 6/6 展开，全部与原版 oracle 一致（`docs/parity-report-alpha39.md`）。
- `PASS` `bash -n scripts/Build-Rust-macOS.sh` 语法与 CI workflow 结构核验；icns 魔数/尺寸校验。
- `PASS` 最终 Windows 安装包重建：alpha.39，21767811 字节，SHA256 AB2EFF4232E7201D4353EA646427EA4DA905F40AFF367533AB68671359938978；隔离静默安装 exit 0、13 模块默认全装、安装后 exe 与 release 一致、206/206 候选、模块子页截图、卸载 exit 0。
- `PASS` 仓库发布与 CI 构建：github.com/AntiO-cps/quicktool-rust（公开，分支 codex/rust-rewrite，scripts/Build-Rust-macOS.sh 保留 100755 可执行位）；公开前清理 3 处本机沙箱绝对路径为 QUICKTOOL_QA_BASE 环境变量（index_parity.rs/capture-index.cjs/benchmark-index.ps1），全量回归复验 168/0 通过。
- `PASS` macOS CI 实际构建（run 42415411312，macos-latest M 系列 runner）：cargo test --lib 通过 → Release 构建 → QuickTool.app ad-hoc 签名（codesign Identifier=com.quicktool.rust）→ DMG 生成 → CI 内 hdiutil 挂载核验（pin 模块、icns、可执行位、签名）→ 构件上传。
- `PASS` DMG 交付与核验：QuickTool-Rust-0.6.0-alpha.39-macos-aarch64.dmg（51,139,710 字节，UDZO/koly 结构合法），本地与 GitHub Release 下载往返 SHA256 一致（5cb74f0bda78a3618f81336fec2af4db89bcbca9a09732848e5d21106bd10aa9），已附于 Release rust-v0.6.0-alpha.39。
- `NOT_RUN` macOS 真机人工验收（下载 DMG 后在 M 系列 Mac 上挂载运行；CI 已完成挂载/签名/内容校验）。

### 结论
- 已完成：一致性量化（覆盖维度 100%，剩余边界显式列出）、macOS 代码级支持与打包/CI 基础设施、最终 Windows 安装包。
- 已完成项补充：macOS 实际安装包已由 CI 产出并发布；仅剩真机人工验收。
- 未完成：macOS 真机人工验收；整体迁移完全一致性仍是长期目标。
- 风险：macOS 上 Windows 专属功能为降级提示而非等价实现；DMG 未经真机验证。

## alpha.40 — IME 组合期间按键误触发修复

### 目标
用户报告：输入法组合中按 Enter 提交候选词时，被应用误当作执行当前候选的回车（原版 Electron 无此问题）。彻查同类按键问题并重新出包。

### 根因
原版运行于浏览器：组合期间 keydown 的 key 为 "Process"（isComposing），页面处理器不视为 Enter。egui-winit 0.31 对 IME 组合期间的 `KeyboardInput` 无任何过滤、原样转发（lib.rs on_keyboard_input 无 is_composing 检查），泄漏的 Enter 到达应用的 keyboard() 即触发 activate(当前选中项)。

### 修改
- `src/ui.rs`：新增 `filter_ime_composition_events`，在 raw_input_hook 对**每个 viewport** 的 RawInput 过滤——凡帧内含 IME 事件（Enabled/Preedit/Commit/Disabled），丢弃无修饰键（无 Ctrl/Mac Cmd）的 Key 按下与全部 Text 事件；含 Ime 的帧意味着组合收尾（Windows 上 Commit 前后伴随 Enable/Disabled），组合中泄漏的 Enter/Esc/方向键/Backspace 全部按原版语义忽略；过滤在探针注入之后执行，使注入场景走生产管线。
- `src/interaction_probe.rs`：新增 4 组回归——IME 组合定位焦点搜索框、预编辑帧不落文本且忽略 Enter、提交帧应用文本（Ime Commit 替换预编辑）并吞掉泄漏 Enter、提交后真实 Enter 正常执行命令；探针扩展至 36 项。
- `Cargo.toml`/`README.md`/`../CHANGELOG.md`：alpha.40 版本与说明。

### 同类问题彻查
- 搜索框（large_input 自定义编辑器）：已有 Preedit/Commit/Disabled 处理与组合中编辑键屏蔽；泄漏 Key/Text 由新过滤器在 raw 层拦截。
- Pin 文本（egui TextEdit）：egui 核心 IME 处理 + Pin 级按键（撤销/Esc/重置）被过滤器覆盖。
- 日期/颜色选择器（子 viewport 事件转发 + 自身按键）、主题下拉、右键菜单（任意按键关闭）：全部经 raw 层过滤。
- 快捷键录制：组合帧的按键按原版语义不参与录制。
- 全局热键（OS RegisterHotKey）：不受 IME 影响。

### 验证
- `PASS` 交互探针 36/36：含新 IME 四场景（组合定位、预编辑不落文本、提交落文本+吞 Enter、真实 Enter 执行）。
- `PASS` 全量回归：cargo test 168/0、clippy -D warnings、fmt、候选差分 206/206、原生展开 6/6。
- `PASS` Windows 安装包重建与隔离静默安装复验（见下）。
- `NOT_RUN` 真实 IME 人工验收（需用户以中文输入法实测 Enter 提交；探针已覆盖事件序列等价物）。

## alpha.41 — 窗口聚焦守护与 IME 组合期失焦保护

### 目标
用户报告：窗口可视时输入不进输入框（"总会失焦"）；期望窗口可视时输入自动聚焦搜索框，失焦即关窗，双平台同步。

### 修改
- `src/ui.rs`：
  - 焦点守护：主面板可见且非设置面板、无选择器/原生取色/菜单/快捷键录制占用焦点时，每帧把焦点归还搜索框（request_focus 下一帧生效），键入始终落在输入框。
  - IME 过滤器精细化：持久 `ime_composing` 状态——Preedit 置位、Commit/Disabled 复位；Windows 的 Enabled（WM_IME_STARTCOMPOSITION）置位、macOS 的 Enabled（IME 切换）不置位；仅组合进行中或含 Preedit/Commit 的帧才丢弃无修饰键 Key/Text，修复 Enabled 帧打字被误吞。
  - `blur_hides` 增加 `!ime_composing` 守卫：组合进行中（候选窗交互造成的短暂失焦）不隐藏窗口；失焦块改为 transition+延迟机制——组合中失焦先延迟（deferred_blur_hide），组合结束且仍失焦时执行隐藏；非组合失焦保持立即隐藏；重新获得焦点清除延迟。
- `src/interaction_probe.rs`：新增 3 组回归（组合定位焦点、Enabled 帧按平台语义、组合中 blur_hides=false/复位后=true），探针 39 项。
- `scripts/Build-Rust-macOS.sh`：SHA256 sidecar 输出裸文件名。
- `Cargo.toml`/`README.md`/`../CHANGELOG.md`：alpha.41。

### 验证
- `PASS` 交互探针 39/39（含 IME 六场景与组合中失焦守卫）。
- `PASS` 全量回归：cargo test 168/0、clippy -D warnings、fmt、候选差分 206/206、原生展开 6/6。
- `PASS` Windows 安装包 alpha.41 重建与隔离静默安装复验（见下）。
- `PASS` macOS CI（macos-latest）构建 alpha.41 DMG、挂载与 ad-hoc 签名校验、Release 双平台发布与往返 SHA256 校验（见下）。
- `NOT_RUN` 真实 IME + 真实鼠标失焦的人工验收（需用户实测：中文输入中点击候选词/点击窗口外）。
