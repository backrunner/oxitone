# VST3 配置阶段的多总线和动态能力协商

本轮补齐配置会话的缺口：原生窗口此前仍走单总线适配，恢复预设时也先用默认参数表
校验。现在工作台、静音配置接口和原生窗口可以恢复实际状态，再处理新的参数表和总线。
运行中图的厂商 GUI、自动化手势和动态换图仍是后续开发，不属于本轮完成范围。

## 实现

- 新增 `configureVst3Plugin(source, options?, host?)`，使用隔离 helper 的 `configure`
  operation，返回实际恢复后的 Vst3Info。共享配置/editor settings，生成独立 JSON schema，
  校验 source/configuration/result 的 class/hash、版本、参数范围与状态上限。
- 原生配置窗口与无窗口接口共用 SilentProcessing。宿主按物理索引保留完整 mono/stereo
  总线表，支持无输入、多个输出及 inactive slot；VST3 零样本处理只刷新事件/参数，不
  提交音频总线或音频指针。AppKit 和插件生命周期调用继续留在 helper 主线程。
- 配置阶段通过已有 vendor service_host_requests 执行 I/O/latency 的停止、deactivate、
  重新协商与恢复处理，重新构建总线缓冲并重查参数。连续重启最多 8 轮；要求替换组件
  或持续变化会失败，不发布部分配置。Apply 前再次 flush，停止后捕获 opaque state。
- 预设参数校验移到 opaque state 恢复之后。预设和显式覆盖先按 ID 合并，只向原生队列
  提交一份最终值；批量控制器值失效在上一轮处理清空队列后同步，避免 4096 参数上限下
  重复入队造成静默丢值。此修改也覆盖现有工程 stream/offline 初始化。
- 工作台重新 Inspect 导入/已 Apply 的预设时恢复实际状态，发布重新解析的参数和值。
  原始恢复预设及路径继续保留；相同配置复用预算，变化的已解析状态计入总预算。
  失败不会修改 source/history 或恢复文件。目录投影发布拆到 vst3-inspection.ts，
  workbench 保持 273 行；原生窗口 236 行，新模块均小于 200 行。
- macOS arm64/Intel CI 增加原生配置 conformance；packed SDK 验证新增操作实际到达
  配套 helper，旧 helper/未知 operation 不会退回默认 inspection。

宿主流程遵循 [Steinberg IComponentHandler 的控制线程要求](https://steinbergmedia.github.io/vst3_doc/vstinterfaces/classSteinberg_1_1Vst_1_1IComponentHandler.html)。
本轮没有改动设备 callback 或图的处理段；新增分配/状态恢复均属于独立配置/prepare 域。

## 验证

- `cargo test --workspace`：613 passed，3 ignored，86 suites；host,stream：52 passed。
- TypeScript 各包及示例共 461 passed。全包首次运行发现旧 fake helper 未实现 configure，
  以及相同恢复配置的重复预算计数；已更新 fake helper 并复用相同配置，CLI 全部 143 项
  及后续包通过。SDK 最新 16 项再次通过。
- `pnpm build`、`pnpm build:wasm`、schema generation、`pnpm format:check`、lint、
  typecheck、`cargo fmt --all --check` 和 `git diff --check` 通过；配套 arm64 helper 已构建。
- 原生动态 fixture 从 1 输出切换到 3 输出，增加参数 ID 7，latency 从 0 变为 64；
  fixture 要求 inactive 协商并拒绝任何非零样本处理。验证状态恢复、processor gain、
  控制器整体刷新、read-only/unknown 拒绝、reload 拒绝与重启风暴上限。
- 第二个原生 fixture 有 4096 个可写参数，满表预设加满表覆盖后只恢复 opaque state，
  确认最后一个参数的实际 DSP 值仍为 0.25。另验证 2入1出、3入3出、0入3出布局。
- 实际 Reaktor 6 Instrument 的 1入8出、FX 的 8入8出、各 3097 个参数均完成无窗口
  配置捕获及重新加载，参数与布局一致；VestiGain 也通过。Reaktor 未加载 ensemble，
  不代表商业资源、授权窗口或声音质量验收。
- VestiGain 原有工程 PCM、gain/mix/bypass/serial/Master/state、frame 73 automation
  与模拟 play/seek/loop/pause/stop 回归通过。1058 blocks，xruns、deadlineMisses、
  nanBlocks、queueDrops、plugin faults 均为 0；后台 block 桶 p95/p99 均为 0.262144 ms。
- 交互脚本返回了 VestiGain 原生 editor Cancel 成功；没有获取可操作的 helper 窗口或
  视觉截图，因此不把此结果算作 Apply、resize、GUI 手势或实时试听验收。

## 控制面基准

Apple M4、Darwin 27，48 kHz / 最大 block 128；配置执行实际处理 0 audio frames，
无音频设备，callback/xrun 指标不适用。动态配置恢复包含新 helper 启动、hash 验证、
状态/总线协商、flush、捕获与退出；3 次预热、30 个样本，p95=9.478 ms、p99=10.043 ms。
执行时本任务未并行运行其他 build/test；未停止其他用户应用，不作跨机器性能保证。

这不是音频 callback 基准，也没有清除上一轮 insert_automation 对照中尚未确认的性能差异。
旧样本全部保留；仍需要同一编译器、已知源码基线和稳定负载的播放性能复核。

原始记录：

- [配置 conformance 与控制面基准](../../benchmarks/results/2026-09-20-vst3-configuration-negotiation.json)
- [实际已安装插件恢复](../../benchmarks/results/2026-09-20-vst3-installed-configuration.json)
- [VestiGain 工程回归](../../benchmarks/results/2026-09-20-vst3-configuration-project.json)

## 继续开发的边界

1. 播放实例的厂商 GUI 和参数 audition；begin/perform/end 手势录制与源码事务。
2. 运行中图的动态 I/O/latency/controller 变更、状态移交、重新编译 PDC 和原子换图。
   配置阶段重协商不会放宽现有运行期 fail-closed 约束。
3. 效果器多输出与更多输入的工程路由、MIDI 输出路由；surround 不在当前 stereo 图中。
4. 厂商 GUI Apply/resize 操作、ensemble/资源恢复、更多商业插件与长时负载、Intel 实机。
5. 原有性能门禁、远端 CI、签名/公证与发布。本轮未提交、发布或部署。
