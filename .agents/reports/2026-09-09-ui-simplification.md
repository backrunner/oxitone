# 插件库与工作区信息精简

本轮根据用户反馈重新组织插件管理器：查找插件只需要名称、类型和选择；
参数编辑属于具体实例，包版本与验证属于按需维护。实现基于当前未提交工作区，
保留已有 Document Service 事务、源码保存与外部插件边界。

## 最终界面

- Plugins 使用 30 px 列表行，主视图仅显示名称和 Instrument/Effect。
  All/Instruments/Effects 直接筛选；搜索仍匹配名称、vendor、package 和 pluginId。
  同名条目才附加 vendor/version，不可用条目保留异常提示。
- 删除参数参考表、统计徽章、卡片、常驻技术信息和缺省占位信息。
  Used in project 按需显示使用位置及 Edit/Open；Details 按需显示版本、来源、验证及包管理。
  Add package 与 rescan 保留在顶部。列表选择本身不新增插件实例。
- 实例配置移入独立内部窗口，标题标明 Channel/Bus、插件和 Insert 槽位。
  输入、滚动、共享范围和编辑目标与目录选择分开。初始高度随参数数量夹紧，
  少参数效果器不再占用大块空白；同插件重开保留用户尺寸。
  数值、默认值、Mix/bypass、局部/共享、重排与串联拆散 review 保持可用。
- 全局传输栏只保留 Undo/Redo/Save 与代码入口，Pattern 作用范围和 Detach 移到钢琴工具栏。
  编排与钢琴标题去掉重复数量；Automation 去掉内部卡片边框、重复插值与 Snap 前缀；
  Mixer 隐藏 0 FX 和静止负无穷 peak。
- 底栏保留文档状态、CPU 与必要音频告警，采样率/延迟/revision/xrun/fault 点击展开。
  插件面板不再常驻 Synced，Copy JSON 归入 Info；错误与 Initial settings 语义继续保留。

## 交互修正

- ⌘/Ctrl+F 搜索；方向键/Home/End 选择，Enter 展开使用位置，Escape 收起。
  搜索及数值输入期间，修饰快捷键不会撤销背后的音乐编辑。
- Escape 优先取消内部窗口拖动，避免被插件列表吞掉。
- 展开详情会缩小列表。GPUI 的普通 scroll-to-item 使用前一帧高度，本轮改为按顶部对齐，
  再由当前布局夹紧，保证选中插件仍可见。详情高度随窗口缩小，340×300 内部窗口仍留出列表。
- 移除废弃 badge/Grid 控件及仅供旧参数参考界面显示的 Rust 投影字段。
  参数元数据仍用于实例配置，未改手写 TS 契约或生成绑定。

## 原生窗口验证

`TMPDIR=/Volumes/BRData/oxitone-prerelease-tmp node scripts/smoke-ui.mjs`：
11 个场景全部通过。每个场景使用临时项目、模拟音频输出、真实 GPUI 键盘或目标 NSEvents，
并保存 TS 后用新进程重开。最后的列表滚动修正另重跑三个插件库场景，增加选中行 bounds 断言，
均通过；记录位于 [library-refinement.json](../../target/ui-review/library-refinement.json)。

| 视图                           | 截图                                                                        |
| ------------------------------ | --------------------------------------------------------------------------- |
| 插件列表，深色 1440×920        | [library-dark](../../target/ui-review/library-dark.png)                     |
| 插件列表，浅色 1060×720        | [library-light-small](../../target/ui-review/library-light-small.png)       |
| 外部插件维护，内部窗口 340×300 | [library-details-small](../../target/ui-review/library-details-small.png)   |
| 独立效果器配置，浅色           | [effects-light-small](../../target/ui-review/effects-light-small.png)       |
| 独立效果器配置，深色           | [effects-dark](../../target/ui-review/effects-dark.png)                     |
| 钢琴工具栏与编辑               | [piano-dark](../../target/ui-review/piano-dark.png)                         |
| Automation                     | [automation-light-small](../../target/ui-review/automation-light-small.png) |
| Browser / Piano / Mixer 共存   | [windows-light-small](../../target/ui-review/windows-light-small.png)       |

已查看上述布局及保存失败弹层。类型筛选实测鼠标命中；搜索输入、键盘选择、使用位置展开、
稳定实例选择与未改变源码/原生投影均有断言。外部效果器测试使用真实 C fixture，
覆盖拆散、数值输入隔离、单实例编辑、重排、typed automation、保存重开和 npm JS/dylib hash 不变。
安装输入测试在 invalid 草稿通过真实 Enter 分发，控制 sender 被捕获，未执行 npm/网络任务。
组合 smoke 按阶段完成后推进；打开 Plugins 后先等待布局，再分发安装按键。

## 检查

- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`git diff --check` 通过。
- `cargo test --workspace`：519 passed、0 failed、2 ignored。
- `smoke-ui.mjs` 与 `smoke-daw.mjs` 的 Node 语法检查通过。
- `node scripts/build-preview.mjs` 已重建 `target/release/Oxitone Preview.app`，
  使用原子替换的新 executable；这是未签名开发应用。
- 聚焦基准 `cargo test --release -p oxitone-preview benchmark_editing_gestures -- --ignored --nocapture`
  通过；六个 UI/control 算法场景、各预热 100 次/采样 1000 次。
  [原始结果](../../benchmarks/results/2026-09-09-ui-simplification.json) 记录 Apple M4、macOS 27、
  Rust 1.98 release 环境；最大 p99 为曲线 4096 点的 569.5 µs。

本轮没有修改音频 callback。算法基准不包含 GPUI/GPU 帧、IPC 或真实设备延迟，
不据此声称整窗帧率提升。NSEvents/截图也不替代 IME、VoiceOver、物理输入或系统全屏验收。
连续参数试听、effective telemetry 和尚未实现的插件生命周期能力不属于本轮 UI 重构完成项。
