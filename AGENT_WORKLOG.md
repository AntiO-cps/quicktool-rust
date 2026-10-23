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
