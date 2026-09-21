# VST3 DAW 播放实例与源码状态接受

## 本轮实现

DAW 实例面板的 Open native editor / Close native editor 控制 Preview 正在使用的 VST3
helper；Use current state 把当前 processor state 和参数作为一条源码配置事务接受。
旧的 editInstance 静音克隆入口及实现移除，catalog 的独立配置 editor 保留。
打开、关闭、试听改参不修改源码；关闭窗口不回滚试听。相同状态重复接受不增加 Undo 条目。

Node Document Service 验证单实例 source boundary、source reads、revision、class/hash，
通过专用 Preview IPC 连接查询当前 graphGeneration/instanceId，再执行控制。N-API 和
Preview 共用 instance control 1 请求验证。专用 RPC 不消费 GPUI 的反向文档请求。
Preview 最多并发 8 个控制任务，后台执行厂商调用，控制线程继续处理 transport 和换图。
请求时限包含收到命令后的排队与线程调度；过期请求在厂商执行前拒绝。完成时再次验证
快照 revision 和图 generation，包含只更换 revision、复用同一音频图时的迟到应答。
实时 callback 未加入 JSON、锁、I/O、N-API 或厂商窗口调用。

捕获成功后仍走既有配置 writer、完整候选比较、原生编译验证和文档历史；保留实例 ID、
宿主 Mix/bypass、其他相同插件与原工厂调用。失败保留源码。没有可隔离 source boundary
时明确拒绝；同一 owner 的共享数组值仍无法猜选，可用独立 addEffect 使用获得准确边界。

4096 参数 Dense 插件发现原有捕获器把每个数值 literal 算作对象边界，导致源代码预算
错误。现在只跳过确定为原始值的 literal，仍捕获配置对象和需要执行的表达式，没有提高
4096 边界上限。完整 Dense 状态接受、Undo/Redo/Save/reopen 已成为两个 macOS CI 架构
的 configuration 检查内容；本机实际执行 arm64。

新增独立模块：Preview 后台控制、GPUI 控制行、Node IPC client、源码实例验证；脚本分为
原生 DAW 生命周期与捕获 conformance。新增手写模块均小于 300 行。既有超长
ProjectDocument 只保留文档私有状态与事务协调，网络和实例验证在外部模块；没有借此次
工作重写其他文档编辑职责。没有提交或清理已有工作区修改。

## 验证结果

- Rust workspace：626 passed、3 ignored、88 suites；VST3 host/stream：58 passed。
- TS workspace：471 passed，其中 CLI 146、protocol 54、native 28、VST3 SDK 16。
- workspace build、Preview debug app、Wasm build、schemas、format、lint、typecheck、
  cargo fmt 和 git diff --check 已执行。一次 TS 全套运行发现 workspace build 清理了 Wasm
  产物，重建 Wasm 后完整重跑通过；没有跳过实际 Wasm 测试。
- Rust 回归覆盖重复快照图复用、旧 generation、未知版本、排队/调度过期、文档反向请求
  不被控制连接抢走。TS 覆盖分片 socket 应答、错误/版本/目标校验、取消、实例边界与重复
  opaque state wrapper 替换。
- [真实 Dense 配置报告](../../benchmarks/results/2026-09-20-vst3-daw-configuration.json)：
  20 项配置检查，包括真实 Preview、DawRunner、DocumentDispatcher、4096 参数 processor
  捕获及持久化；零音频帧、无输出设备。
- [真实 VestiGain DAW 报告](../../benchmarks/results/2026-09-20-vst3-live-daw-editor.json)：
  两个共享 preset 的独立实例；播放时改参、原生窗口开关、源码不变、捕获接受、相同状态
  no-op、旧图拒绝、Undo/Redo/Save/reopen 全通过。模拟 sink：48 kHz / 128 frames，
  xruns=0、pluginFaults=0。接受后的 WAV 比初始值降低 18 dB，48,000 个声道样本的最大
  PCM 误差约 3.82e-9。此为短时 smoke，不是持续 GUI 负载或听音验收。
- 原生窗口由实际控制接口打开/关闭，未进行可视点击或缩放验收。CUA 当前返回 Mac locked，
  自动解锁失败；不把程序化 editorOpen 状态当作可视验收，也未通过其他工具绕过锁屏。

## 基准与证据边界

停止本轮 build/test 后分别运行 preview 与 insert_automation，1 秒预热、3 秒测量、
30 个样本。Apple M4 / Darwin 27，48 kHz / 128 frames，bench 优化构建，无音频设备。
[完整 estimates、样本、编译器和二进制 hash](../../benchmarks/results/2026-09-20-vst3-daw-benchmark.json)。

| 场景                    |       mean | 归一化批次 p95 | 归一化批次 p99 |
| ----------------------- | ---------: | -------------: | -------------: |
| Preview baseline        |  38.329 µs |      38.803 µs |      39.180 µs |
| Preview telemetry       |  40.018 µs |      41.391 µs |      41.652 µs |
| Insert render 128       | 173.894 µs |     180.634 µs |     186.121 µs |
| Typed insert render 128 | 172.564 µs |     177.887 µs |     183.562 µs |

上述 p95/p99 来自 Criterion 批次均摊时间，设备 callback p95/p99 未测。插入自动化相对
上轮无显著变化。Preview telemetry 相对较早 Criterion 缓存基线提示约 +8.8% 回退，保留
比较数据；缺少可重建的同源码、同环境对照，不能归因本轮或宣布性能验收完成。

## 仍需推进

1. 厂商 GUI begin/perform/end 手势、准确时间坐标、自动化录制和有界事件溢出处理。
2. 动态 I/O、latency、参数表变化的状态交接、PDC 重编译与原子发布；目前仍使旧流失效。
3. 效果器多输出、更多输入选择、MIDI 输出与 surround 路由。
4. 原生窗口真实点击/Apply/Cancel/缩放验收及播放中的 GUI 负载；当前 UI/DSP 仍在
   helper 中串行执行，慢厂商调用可能导致音频超时。
5. 商业插件的资源/ensemble、长加载和 soak、Intel 实际执行、签名/公证与发行验证。
6. 受控性能基线、回退定位和完整兼容性矩阵。

DAW 播放实例控制与显式状态接受已接通，“完整 VST3 接入”目标仍未完成。
