# DAW 日常工程操作（2026-09-09）

本轮在紧凑插件库／独立实例配置的基础上接入 Mixer、静态 BPM、Playlist 片段操作和
插件实例选择。行为契约见 [23-daw-controls](../docs/23-daw-controls.md)，用户操作见
[Preview / DAW](../../docs/preview.md)。这份记录不关闭整个 source/DAW 设计矩阵。

## 已落地

- Mixer Channel/Bus/Master 的 level、pan/balance、mute、solo 可写回；推子／声像
  支持 Shift 精调、双击复位和 Escape 取消，释放只提交一次。BPM 输入支持 Enter／Escape，
  不覆盖 tempo automation 或多段 tempo map。Track 标题色条切换 enabled。
- Pattern/Sample/Automation placements 支持选择、移动、Shift 拖动复制、⌘D、
  Delete/Backspace、M；右边缘调整 Pattern/Automation 长度或按当前 Sample tempoSync
  适配播放窗口。复制保留 placement 配置并共享音乐定义；Pattern lastBeat 随起点平移。
- Mixer Chain 采用紧凑插件行，提供打开、替换、删除和 Add effect。Library 复用为匹配
  Instrument/Effect 类型的 picker，按 Enter 确认，失败保留选择。普通库仍只有名称／类型，
  参数仅在实例配置中显示。没有文档连接的 Preview 不展示 Add effect。
- 内置与验证通过的 npm 插件共用实例流程。新增插件配置保留来源映射，可接着修改参数；
  效果器重排支持最终 Project 配置新增的实例。面板跟随稳定实例，重排后保持对象，替换／
  删除后旧面板不转而控制新插件。已绑定 automation 的实例不能被隐式删除或替换。
- 缺依赖导致的 invalid 草稿允许安装／修复；任务失败保持原 invalid 状态，成功重新求值。

## 源码与正确性

所有控件经 Node Document Service 的版本化事务接受，再投影到 GPUI／原生引擎。
Project.configure 与 Project.arrange 使用 builder 顺序，避免 canonical ID 排序误选对象。
相邻纯 JSON 标量调用合并、相邻效果器排列合成；无变化不生成 revision。
原表达式只求值一次，保留 import/export、局部变量和动态调用。写回不添加实体 ID。

新插件注册写为当前工程的 withPluginRegistration，路径基于 import.meta.dirname 与
正常 node_modules 包路径，避免持久化 pnpm store 的实际路径。验证包括 metadata、动态库、
hash、现有签名策略、依赖读集与完整 expected frame；不自动放宽 signed-only。
音乐编辑不更改 npm 文件，真实外部 C fixture 的 JS／动态库字节在保存重开后保持不变。

候选代码重新执行、原生编译与完整工程比较成功后才接受。校验仅允许新创建的 clip／plugin
身份别名，保留已有身份、automation 和无关工程设置。TS 与 DAW 共用 Undo/Redo/Save。
本轮没有修改实时音频 callback；手势中的本地数值预览不等同于连续参数试听。

## 回归重点

- builder 顺序与 canonical ID 顺序不同时仍修改正确 Channel；标量合并与 no-op。
- 片段复制保留 transpose/velocity/sample 设置；移动修正 lastBeat，同轨移动不重排 membership。
- 插件添加 → 修改新实例参数 → 重排 → 替换／删除 → Undo/Redo → Save → 新进程重开。
- 外部插件逻辑包路径、签名策略、自动化保护、模块／库 hash 和注册帧范围。
- 真实 dispatcher 的 invalid 草稿修复与任务失败状态保持。
- GPUI 推子／声像／静音的实测点击位置，异步 NSEvents 分帧按下／抬起，Escape、复制／撤销、
  picker Enter、配置输入隔离、BPM 输入及保存重开。

源码事务基准（Apple M4，10 次迭代）记录于
[source-daw benchmark](../../benchmarks/results/2026-09-09-source-daw.json)：Mixer／Playlist
事务 p95 约 163–172 ms，保存 p95 约 52 ms；不连接音频设备。GPUI 编辑手势 release 基准
记录于 [daw-editing benchmark](../../benchmarks/results/2026-09-09-daw-editing.json)。

## 尚未完成的边界

保持相位的 split／左边缘源偏移依赖 musical origin／source window，不能用复制裁切替代。
多片段框选／剪贴板、Playlist 画笔／边缘自动滚动、跨 clip automation、可编辑 routing/send、
Sample 资产操作和有资源／状态要求的插件创建向导仍待实现。当前缺资源的插件由原生编译
拒绝，不报告虚假成功。Pattern/Sample 的局部 Track tempo 移动、复制、resize 暂时拒绝。

连续参数试听与 touch/latch、ABI 2 resources/state、并行 rack／macros、完整包存储回滚、
完整 resolver/IDE/资产保存和大型工程增量性能仍是后续门禁。工程回写目前要求捕获到最终
Project return/default export；这不等同于任意导出封装均可编辑。
