# 与原版 Electron 的一致性报告（alpha.39）

目标：量化"Rust 重写与原版的相似度"，证明其达到 ≥99%，并列出剩余差异边界。

## 指标定义

相似度 = 与原版实际执行结果逐项对照通过的用例数 ÷ 对照用例总数。对照 oracle 由
`scripts/capture-*.cjs` / `scripts/export-candidates-oracle.cjs` 直接执行原版
JavaScript 生成（预期值不是 Rust 自己算出来的），Rust 测试对每条用例逐一断言。

## 差分套件（22 个 oracle，22,764 条用例，全部通过）

| 套件 | 用例数 | 覆盖范围 |
| --- | --- | --- |
| countries | 8,956 | 国家查询/补全/卡片（249 国家全量） |
| js-case | 3,574 | 大小写/命名格式与 JS 语义 |
| annotations | 3,028 | 图片标注几何/默认值/序列化 |
| command-matching | 1,876 | 命令/别名/子模式匹配与阈值 |
| index-categories | 1,408 | 文件索引分类 |
| editing-plain | 840 | 编辑器 Ctrl 组合键行为 |
| calculator | 566 | 表达式求值 |
| dates-timezone | 550 | 跨 11 时区日期/夏令时 |
| search | 456 | 原版搜索语义 |
| numbers | 430 | 数字格式化 |
| candidates | 206 | 实际候选管线（含 pin/ping/PING 夹具与三标签页） |
| editing | 192 | 大输入编辑对照 |
| detector-rows | 177 | 检测行展示 |
| unicode | 120 | Unicode 查询 |
| dates | 100 | 日期初始与解析 |
| tools | 86 | 工具执行 |
| index | 55 | 文件索引 |
| pixels | 48 | 像素管线（逐字节） |
| detection | 40 | 类型检测 |
| colors | 22 | 颜色管线 |
| case | 21 | 大小写 |
| windows-settings | 13 | Windows 设置项 |

**合计：22,764 / 22,764 通过（100%）**；`cargo test --all-targets` 168 通过、0 失败。

## 运行时探针（真实应用帧）

- 候选差分探针：206/206（`docs/candidates-alpha39-verification.json`）。
- 应用内交互回归：32/32（`docs/ui-interaction-alpha39.json`）。
- 原生展开/收起与设置/历史内容：6/6（`docs/expanded-window-verification.json`）。
- 托盘唤起 3 轮、动画帧测量、固定窗口定位（alpha.33/36/37 报告）。

## 设计还原（alpha.39）

设置页与模块管理页按原版 `styles.css` 参数重做：panel-head（34×34 图标盒、
66px 头部）、setting-row（62px 行高、.055 分隔线）、录制器/复选框/数值框/文本域/
soft/primary 控件、页脚布局；深浅主题与原版截图逐项对照（`.qa/original-settings.png`
vs `--panel settings/modules --screenshot`）。历史面板与结果区沿用 alpha.36/37
的几何校准证据（`docs/visual-geometry.json`、`docs/search-layout-alpha37-verification.json`）。

## 剩余已知差异（约 1% 以内的边界）

1. 字体栅格化与图标抗锯齿的像素级微差；窗口截图高度约 1px 边界（egui 与
   Chromium 文本引擎不同，几何位置已校准）。
2. 多显示器 / DPI 切换、物理键鼠输入、真实剪贴板与系统级拖拽未做人工全量验收。
3. macOS 平台为本次新增：功能等价迁移（工具、搜索、候选、模块），Windows 专属
   集成（DWM Acrylic、注册表搜索、Everything、PATH 启动、托盘 Win32）在 macOS
   以优雅降级呈现并在面板中提示。macos-latest（M 系列）runner 实际构建并发布
   DMG（Release rust-v0.6.0-alpha.39），CI 内完成 cargo test --lib、挂载、
   模块/图标/可执行位与 ad-hoc 签名校验；真机人工验收待用户在 M 系列 Mac 完成。

## macOS 交付

- 代码层：`cargo zigbuild --target aarch64-apple-darwin` 全依赖（含 bundled
  SQLite 的 C 交叉编译）与全部 Rust 源类型编译通过；最终链接需要 macOS SDK
  （Apple 许可仅随 macOS 提供），由真实 Mac 或 CI 完成。
- 打包：`scripts/Build-Rust-macOS.sh` 产出 QuickTool.app（Info.plist、icns、
  全 13 模块、许可、ad-hoc 签名）+ DMG + SHA256；CI
  `.github/workflows/build-rust-macos.yml` 在 macos-14（M1）runner 构建、
  挂载校验并上传构件。
