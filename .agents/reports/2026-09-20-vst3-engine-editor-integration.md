# VST3 工程、编辑器与分发接入记录

此增量延续 09-19 的审计，保留已有未提交工作。当前已连接 mono/stereo VST3 的工程音源、
轨道/bus/Master 效果器、自动化、离线导出、模拟播放、源码分配与配置保存。仍不能宣布
全部 VST3 兼容或完整验收：原生厂商窗口的交互尚受锁屏阻挡，多总线等能力也未实现。
没有打开系统音频设备、试听、提交代码或发布 npm。

## 已实现

- Engine 协议 1.3、独立 VST3 registrationVersion 1。Project/Session/native/Preview 重放
  本地注册，class ID 与 bundle SHA-256 固定身份；旧版本拒绝新 state/VST3 能力，保留恢复数据。
- RenderGraph 区分 Realtime、Isolated 和 Offline 执行域。VST3 处理只在后台/离线线程
  等待 helper，设备回调只复制已完成 PCM；direct 请求明确回退到 buffered。
- instrument/channel insert/bus/Master 使用同一事件、宿主干湿/bypass、路由与 PDC。
  PDC 只计插件固有延迟，不把 SDK 的固定调度额外叠加。独立测试验证 17 帧插件延迟的
  并行/串联、dry/wet 和 Master 对齐，错误整块静音并停止播放。
- 真实 ProcessContext 包含工程/连续帧、tempo map、原始拍号、quarter-note/bar 与 cycle。
  短块只推进真实帧数；stream 5 的 reset 与下一块位置/事件/PCM 一起传递，helper 用
  setProcessing(false/true) 清空 DSP，不再为正常 seek/loop 重启进程。故障恢复仍显式重建。
- SDK 系统目录扫描、多 class 枚举、DAW Scan installed VST3 → 选择 bundle → Discover
  plugins → Assign。扫描不加载二进制，不改源码 revision；枚举/检查隔离执行并复核 hash。
- macOS arm64/x64 optional helper 包、自动解析、显式覆盖错误不回退、锁定构建和许可证清单。
  arm64 真实打包后在仓库外解析/运行已通过；Intel 实际运行仍依赖其 CI runner。
- 独立 AppKit/IPlugView 配置编辑器，主线程 event loop、厂商 resize、Apply/Cancel、关闭前
  detach、取消/超时杀死并回收。零样本处理同步 GUI 参数，不打开音频设备。
- 工作台 Apply 更新可保存的会话 configuration；已插入插件使用准确实例 state/参数。
  Apply 经 source reads/revision 检查与原生候选编译，用 PluginConfig.withState 写回。
  保留实例 ID、自动化、宿主设置及其他相同插件；重复 Apply 不累积旧 opaque state。
  Cancel 不改源文件或 revision，Undo/Redo/Save 使用原事务机制。

## 自动化与实机证据

- `pnpm build`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `cargo test --workspace`：605 通过、3 ignored，85 个 test/doc-test suite，无失败。
- `cargo test -p oxitone-vst3-host --features host,stream`：49 通过，覆盖协议、短块、reset、
  故障、取消/reap 和零分配/释放端口。独立 client 不引入插件 loader。
- 编辑 SDK Apply/Cancel/身份错误/取消、工作台状态预算与刷新、源码单实例与重复 Apply、
  扫描陈旧结果/取消原子性测试通过。完整 TS 回归首次在 Preview socket 测试遇到
  EADDRNOTAVAIL 并超时；单独及完整 CLI 复核通过。后续 Web 测试因构建清理了 Wasm 产物
  报 ENOENT；执行 pnpm build:wasm 后补测通过，未改测试放宽标准。分阶段完成全部
  package/example 测试，共 454 项通过，其中 CLI 142、core 187、VST3 15、protocol 48。
  Web 9 项包含所有生产效果器/IR 与 native PCM 对齐及处理零分配、释放、内存增长检查。
- VestiGain 真实工程 PCM 覆盖插件 bypass、-6 dB、mix=0.25、host bypass、两级串联、Master、
  保存恢复与 frame 73 自动化。数值误差在报告内，自动化断言保留 Master limiter 的 FIR 边缘。
- VestiMIDISynth 真实 DAW 分配、注册/state 写回、catalog refresh 去重、Undo/Redo/Save/reopen、
  MIDI note-on/off 与全工程 WAV 已通过。真实 GPUI 捕获显示厂商名称和参数当前值。
- 扩大本机兼容检查：Reaktor 6 6.5.0 的两个 audio class（Reaktor 6 / Reaktor 6 FX）均能
  枚举，inspection 因多总线返回 PluginCapabilityUnsupported；没有静默截断或假装支持。

环境：Apple M4、Darwin 27.0.0、Rust 1.98.1 / LLVM 22.1.8、release、48 kHz、128 帧、stereo。
设备为模拟 sink 或无设备。硬件 callback p95/p99、CPU 总占用率与听音均未测。

| 验证                                                                                              | 结果                                                                                   | 范围                                                   |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| [transport/reset 原始结果](../../benchmarks/results/2026-09-20-vst3-transport-reset.json)         | 400 块、26,129 帧、79 次 reset；IPC p95/p99 0.1395/0.225292 ms                         | 实际 conformance processor 时钟与 PCM，非设备 callback |
| [工程 reset 后](../../benchmarks/results/2026-09-20-vst3-project-reset.json)                      | 1,058 个后台块；xruns/deadlineMisses/nanBlocks/faults 均 0；后台 block p99 0.262144 ms | 实际 VestiGain + 模拟 sink，非任意负载保证             |
| [insert automation 基准原始样本](../../benchmarks/results/2026-09-20-vst3-insert-automation.json) | 128 帧渲染约 189 µs，typed 约 186 µs                                                   | 内置图执行回归基准，无 VST3 helper/设备                |

正常循环此前采用进程重建，一次短回归出现约 67 ms 后台块与 1 次 deadline miss。
改为标准 DSP reset 后，上述更长短循环回归无 miss；不据此宣称第三方插件均正确实现 reset。
最终代码重建后的[工程复核](../../benchmarks/results/2026-09-20-vst3-project-final.json)再次
得到 1,058 块、全部错误计数 0、后台块 p99 0.262144 ms。脚本现在写完报告后明确断言
xruns/deadlineMisses/nanBlocks/queueDrops 均为 0，失败不丢弃测量报告。

Criterion 历史对照早于已有未提交工作，不是干净的 VST3 前后对照。首次输出把 compile
约 +10.5%、typed compile 约 +6.0%、typed render 约 +7.3% 标记为回归；原始样本已保留，
不能将该差异全部归因于 VST3 或声明没有性能回归。普通 render 的历史差异未显著。
[最终代码复核](../../benchmarks/results/2026-09-20-vst3-insert-automation-final.json)保留完整
Criterion 输出和样本：compile 44.826 µs、resolve 55.148 ns、render 191.92 µs、typed compile
45.196 µs、typed render 180.83 µs。与首轮相比 render 未显著变化；原始历史回归记录不覆盖。

## 尚未完成的验收与功能范围

1. Mac 处于锁屏，CUA 返回无法解锁。真实厂商 editor 启动后未收到鼠标操作，180 秒后
   watchdog 终止；这只验证进程存活/超时回收，不是 Apply/Cancel/缩放通过。解锁后需要
   用 `scripts/smoke-vst3-editor.mjs` 验证 Apply、Cancel、窗口关闭与 resize，再验证实例保存重开。
2. 当前原生 editor 是静音配置会话，Apply 后重新编译。未实现运行实例的实时厂商 GUI
   参数试听、gesture 自动化录制；现有通用参数面板也沿用初始配置事务。
3. 多输入/多输出 bus、sidechain、surround、MIDI 输出与动态 layout/latency 协商仍显式拒绝。
   Reaktor 是已确认的兼容性缺口，不能仅用 VestiGain/VestiMIDISynth 作为全量支持证明。
4. Intel runner、更多商业插件/资源恢复/压力场景、真实硬件 callback 与听音尚未验收。
   npm 仅完成本地产物与 CI 流程，未发布、签名公证或操作远端工作流。

## 模块边界与复现

执行域与 RenderGraph 处理已拆到 graph/execution、render/graph 子模块；VST3 factory、
metadata、instance/events 各自聚焦；配置 source writer 与实例 editor 放在 CLI plugins/editing。
AppKit window 约 231 行、工作台 298 行、目录扫描 UI 81 行。既有超长 ProjectDocument 仅组织
语义事务，新增具体行为在专属模块；generated schemas/bindings 不手改。vendored MIT
loader 的窄 ProcessPosition/reset 扩展与来源保留在 vendor/vst3-host/OXITONE.md。

```sh
pnpm build
node scripts/build-preview.mjs --debug
node scripts/smoke-vst3-package.mjs
node scripts/smoke-vst3-transport.mjs
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3-project.mjs
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 OXITONE_VST3_INSTRUMENT=/path/VestiMIDISynth.vst3 node scripts/smoke-vst3-daw.mjs
cargo bench -p oxitone-bench --bench insert_automation -- --warm-up-time 1 --measurement-time 3 --sample-size 30
```

交互脚本显式设 fixture 后运行 `node scripts/smoke-vst3-editor.mjs`；点击 Apply 应返回
configuration，`--cancel` 运行点击 Cancel/关闭应返回 null。脚本不会打开系统音频输出。
