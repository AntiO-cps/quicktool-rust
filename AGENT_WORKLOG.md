## 全 Rust 重写（独立目录与分支）

| 字段 | 内容 |
| --- | --- |
| 状态 | `部分完成` |
| 时间 | `2026-10-20T00:07:15+08:00` → `2026-10-20T23:25:52+08:00`（本阶段收尾） |
| 最后更新 | `2026-10-20T23:27:27+08:00` |
| Git | 独立目录 `codex/rust-rewrite` · 无初始提交 → `85aee9f1f3c6e3cd6b4764970ff26a5c147976bd`（阶段代码；日志另行提交）；原目录 `master` 无提交，原有文件仍未跟踪 |

### 目标
在独立目录将 UI、交互与业务逻辑全部迁移至 Rust，保持原版一致，并测量性能。

### 修改
- `Cargo.toml`、`Cargo.lock`、`src/`：原生 Rust UI、工具核心、索引与 Windows 集成、托盘、Gaussian Blur/Glow、文字/图片 Pin、标注、PNG/PSD、撤回与历史/设置；导出采用原子替换。
- `assets/`、`vendor/epaint/`：原版字体/图标/旗帜/数据与许可；最小可变字体轴补丁。
- `tests/`、`reference/`、`scripts/`：原版输出与几何差分、原生测试、构建脚本、截图/性能探针及原版哈希校验。
- `docs/`、`README.md`：迁移验收、已知未完成项、像素及三轮应用性能数据。
- `.gitattributes`、`.gitignore`：规范文本换行；排除构建缓存、隔离测试数据与发布产物；缓存位于 C 盘本任务目录。
- 本仓库及原项目根目录 `AGENT_WORKLOG.md`：沿用同一任务条目持续记录。
- 仓库本地 Git 身份：按用户提供值设为 codex <codex@example.com>；未修改全局身份。

### 验证
- `PASS` `cargo test --offline --all-targets -- --nocapture`：72 项通过、3 项显式跳过。包括 566 计算器、430 数字格式、120 Unicode、100 日期、86 初始核心、86 工具输出、40 检测评分、22 组颜色、48 组滤镜、2860 组几何以及 PSD/14 类 Canvas 对照。
- `PASS` `cargo clippy --offline --all-targets -- -D warnings`：无警告。
- `PASS` `cargo fmt --all -- --check`：通过。
- `PASS` `cargo build --offline --release --bin quicktool-rust`：发布构建成功，程序 63516160 字节。
- `PASS` `scripts/Build-Rust.ps1 -Offline -SkipBuild`：生成 `dist/QuickTool.exe`、资源、文档与 SHA256 清单。
- `PASS` 可移植目录运行：进制结果、国家卡片/国旗、设置截图成功，进程正常退出。
- `PASS` 三轮隔离性能采样：启动中位 2.445s → 0.699s，空闲工作集约 855MiB → 194MiB；同十组查询。测量边界与原始样本见 `docs/app-benchmark.json`；首次 Unicode 查询较慢。
- `PASS` `node scripts/verify-original.cjs`：589/589 原版源码哈希未改变。
- `PASS` 敏感信息模式扫描：任务源码/脚本/测试/文档未发现匹配。
- `PASS` 暂存 diff 检查：任务源码无空白错误；许可原文两处尾空格及参考原文末尾空行保持原样。构建产物与隔离数据均未暂存。
- `SKIPPED` 3 项：有意供子进程使用的 sleep 夹具；需要 Windows Search 服务的真实查询；需要私有 window station 权限的隔离剪贴板。未触碰真实用户剪贴板。
- `FAILED` Pin/效果面板截图：30 秒超时；已定位 eframe Glow immediate viewport 不处理 Screenshot action，未将此项计为通过。
- `NOT_RUN` 手工鼠标/键盘全流程：computer-use 访问等待超时；发布截图仅验证显示，不代替交互验收。

### 结论
- 已完成：独立目录/分支、主要原生 Rust 迁移、上述测试、发布目录及性能测量；本阶段提交保存可编译状态。
- 未完成：UI/交互完全一致；Pin 菜单/缩放/草稿清空细节；通用图片结果历史资源；旧数据导入。详见 `docs/migration.md`。
- 未验证：Pin 独立窗口截图与手工交互、多屏/DPI、真实启动/管理员/Regedit/剪贴板操作、Everything 与 Windows Search 非空返回、大目录索引/拖拽帧耗时。
- 风险：目标状态为额度受限，整体目标尚未完成；当前存在已知视觉/行为差异及首次 Unicode 查询性能回退，不能标为等价替代版本。

