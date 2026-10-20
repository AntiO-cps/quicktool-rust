## 全 Rust 重写（独立目录与分支）

| 字段 | 内容 |
| --- | --- |
| 状态 | `进行中` |
| 时间 | `2026-10-20T00:07:15+08:00` → 进行中 |
| 最后更新 | `2026-10-21T01:22:59+08:00` |
| Git | 独立目录 `codex/rust-rewrite` · 无初始提交 → `c4b7b6b7c8da0c4e823842285ef1eb521ce7b583`（本轮已验证代码；继续实现弹层等未完成项）；原目录 `master` 无提交，原有文件仍未跟踪 |

### 目标
在独立目录将 UI、交互与业务逻辑全部迁移至 Rust，保持原版一致，并测量性能。

### 修改
- `Cargo.toml`、`Cargo.lock`、`src/`：原生 Rust UI、工具核心、索引与 Windows 集成、托盘、Gaussian Blur/Glow、文字/图片 Pin、标注、PNG/PSD、撤回与历史/设置；导出采用原子替换。
- `assets/`、`vendor/epaint/`、`vendor/eframe/`：原字体/图标/旗帜与许可、可变字体轴、独立窗口Screenshot/Cut/Copy/Paste命令补丁；新增零分配Unicode二进制索引。
- `tests/`、`reference/`、`scripts/`：原版输出与几何差分、原生测试、构建脚本、截图/性能探针及原版哈希校验。
- `docs/`、`README.md`：迁移验收、已知未完成项、像素及三轮应用性能数据。
- `.gitattributes`、`.gitignore`：规范文本换行；排除构建缓存、隔离测试数据与发布产物；缓存位于 C 盘本任务目录。
- 本仓库及原项目根目录 `AGENT_WORKLOG.md`：沿用同一任务条目持续记录。
- 仓库本地 Git 身份：按用户提供值设为 codex <codex@example.com>；未修改全局身份。

### 验证
- `PASS` `cargo test --offline --all-targets -- --nocapture`：95 项通过、3 项显式跳过（2026-10-21 本轮整合）。包括 566 计算器、430 数字格式、120 Unicode、100 日期、86 初始核心、86 工具输出、40 检测评分、22 组颜色、48 组滤镜、2860 组几何以及 PSD/14 类 Canvas 对照。
- `PASS` `cargo clippy --offline --all-targets -- -D warnings`：无警告。
- `PASS` `cargo fmt --all -- --check`：通过。
- `PASS` `cargo build --offline --release --bin quicktool-rust`：发布构建成功，程序 61423104 字节。
- `PASS` `scripts/Build-Rust.ps1 -Offline -SkipBuild`：生成 `dist/QuickTool.exe`、资源、文档与 SHA256 清单。
- `PASS` 可移植目录运行：进制结果、国家卡片/国旗、设置截图成功，进程正常退出。
- `PASS` 三轮隔离性能采样：启动中位 3.465s → 0.842s，空闲工作集约 847MiB → 234MiB；同十组查询。测量边界与原始样本见 `docs/app-benchmark.json`；首次 Unicode 重建从255.56ms降至9.22ms；此前数据保留在before-index报告。
- `PASS` `node scripts/verify-original.cjs`：589/589 原版源码哈希未改变。
- `PASS` 敏感信息模式扫描：任务源码/脚本/测试/文档未发现匹配。
- `PASS` 暂存 diff 检查：任务源码无空白错误；许可原文两处尾空格及参考原文末尾空行保持原样。构建产物与隔离数据均未暂存。
- `SKIPPED` 3 项：有意供子进程使用的 sleep 夹具；需要 Windows Search 服务的真实查询；需要私有 window station 权限的隔离剪贴板。未触碰真实用户剪贴板。
- `PASS` Pin/效果面板截图：本地 eframe 修复后文字、图片、效果面板均实际生成，原版同内容参考亦已采集；仍存在字体/SVG栅格差异。
- `NOT_RUN` 手工鼠标/键盘全流程：computer-use 应用授权再次超时，隔离验证进程已按CIM绝对路径核验并关闭；事件事务测试与截图不代替完整OS交互验收。

- `PASS` 143668 Unicode名称/类别逐条对照及120组原版输出；新版只读索引取消首次9.5MB JSON解析。
- `PASS` 普通图片历史：后台资产写入、跨Pin去重、损坏资源保护、失败回滚、旧记录兼容；全部存储测试通过。
- `PASS` 旧数据导入：8组合成夹具及发布版CLI实际导入/拒绝覆盖；旧源hash保持不变，错误原文保留。
- `PASS` Pin新增5项状态测试：清空不回加、缩放保持线宽/马赛克、Unicode选区、恢复边界/取整、动态默认重置。

- `PASS` 日期后续差分：192 条跨 6 个 IANA 时区完整输出及 4 项边界测试通过；新增依赖已下载。此项为当前工作区验证，尚未纳入上一整合提交。
- `FAILED` 索引扩展差分基线：52 组查询中 35 处输出差异、实扫 8 组中 5 处差异；已修正源码，等待重跑。

### 结论
- 已完成：独立目录/分支、主要原生 Rust 迁移、上述测试、发布目录及性能测量；本阶段提交保存可编译状态。
- 未完成：UI/交互完全一致；Chromium日期/颜色弹层、屏幕滴管、大段Base64解码worker及多DPI/完整输入验证。Pin交互与图片历史、旧数据导入已修复并验证。详见 `docs/migration.md`。
- 未验证：手工交互、多屏/DPI、真实启动/管理员/Regedit/剪贴板操作、Everything 与 Windows Search 非空返回、大目录索引/拖拽帧耗时。
- 风险：目标已恢复执行，整体目标尚未完成；当前存在已知视觉/行为差异及尚未完成的原生弹层/完整输入验收，不能标为等价替代版本。
