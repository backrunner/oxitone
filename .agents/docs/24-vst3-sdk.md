# 本地 VST3 SDK、工程实例与 DAW 工作台

## 配置后的实例能力与恢复

工程编译在控制线程按 instanceId 准备配置后的 VST3 工厂。参数表、总线布局和输出数量
来自恢复 state/parameters 后的实际 helper，而不是同类插件的默认注册表；同一 class/hash
的其他实例仍使用自己的配置。完整参数、automation 和路由验证使用该实例能力，prepare
再次核对布局并以实际 latency 重算全图 PDC。失败不发布候选图，也不改变注册表或冻结状态。
此能力只扩展 Rust 原生 factory，不改变 C ABI 1；不向音频 callback 增加工作。

动态变更后使用 capture 获取冻结配置，通过 SDK 更新拥有者配置并 Session.update，或
DAW 的 Use current state 文档事务恢复。后者仍支持 Undo/Redo/Save/reopen；不隐式重写
自动化；捕获的完整参数表替换初始参数，已移除的参数或总线仍被 automation/路由引用时
明确拒绝，用户可修正这些引用后重试。
新图按既有 block boundary 发布，旧 generation 失效，不承诺旧尾音跨图延续。

## DAW 录制事务

实例面板按参数选择 Touch/Write，Document startRecording 绑定单个配置使用、源码 revision、
read set 与 Preview graph generation；每份文档最多一个录制。播放仍由原 transport 控制。
stopRecording 在播放边界停止、收齐 take，并提交一次完整候选源码事务；cancelRecording
使用录制任务 ID，可绕过排队中的停止请求。迟到响应不能提交已取消任务，也不能重定向实例。
文档换版/关闭使正在采集的旧任务失效并清理；失败不改源码。已完整采集但写入失败的 take
保留在会话中，允许同一未变化 revision 重试接受或显式取消；不把它冒充已保存的数据。

SDK Project.recordAutomation(edit) 按 owner/slot 和插件版本解析目标，先验证完整 take，
再创建独立片段。DAW 以同一规划产生普通 AutomationSource、param.automate 和
createAutomationClip 作者调用，源码不打印 instanceId/captureId。局部 Source 定义各自
有可编辑边界；包裹表达式返回同一 Project，保留原表达式一次求值和注册。
结果为独立 Playlist 自动化片段及 Track，使用 Engine 1.5 priority，详见
[07](07-automation-spec.md)。原 lanes/共享来源不重写，候选全工程比较只允许增加指定
参数的录制片段。成功支持一次 Undo/Redo 与普通 Save；不自动保存或烘焙原生成器。

## 厂商手势日志

### 自动化录制

startRecording(mode: touch|write, parameterIds) 复用同一 helper 的 captureId/分页/停止/取消
生命周期，与纯观察 startEdits 互斥。最多选 32 个按升序排列的不重复可写且可自动化参数。
页附 recording(mode,parameterIds,sampleRate)。额外 sample 事件表示宿主实际应用的值及
frames 区间，不冒充厂商回调；原 begin/value/end 仍保留。每个 sample 覆盖该包的
[projectBeat, projectBeat + frames * tempo / (60 * sampleRate))，仅播放包产生。

Write 从启动到停止边界覆盖所选参数；Touch 在 begin 到 end 期间覆盖，含 end 所在
音频块，下一块恢复已有自动化。同块多次 value 使用最后值；不覆盖其他参数或音符。
Touch 的重复 begin、未开始的 value/end 使采集失败并解除覆盖；Write 允许无 brackets
的 value。新回调只影响尚未处理的音频块。暂停时保留待处理手势但不录入区间，seek/loop
继续记录实际收到的包坐标。stop 边界解除覆盖；末个已录制 sample 的结束是下一包开始。
失败或 discard 解除覆盖，后续原自动化重新生效；不宣称回滚之前已经播放的声音。

SDK recorder 定期收取并确认页，停止后返回有界 take，失败不返回部分结果。将 take 的
sample 区间合并为 step source 覆盖：后录入的重叠区间优先，未触及区间保留原 source；
同值相邻区间合并，不用采样整个旧生成器替代其定义。来源是 Project beat，不自动猜测
有 loop/playlist 的 lane 本地域。DAW 使用本文开头的独立 Playlist 层完成映射与源码事务。

`Session.recordVst3Automation(target, {mode, parameterIds})` 启动后每 20 ms 收取分页，
积压页立即续读。`recorder.stop({timeoutMs?})` 默认允许 5 秒收集停止边界，范围 1…600000 ms；
超过截止时间到达的页拒绝接受，清理还可能等待已有原生请求结束。`cancel()` 等待清理完成，
期间到达的完成页也不能再返回 take。`active` 与 `error` 提供采集状态；失败没有部分 take。
清理只对原 captureId/generation 发送 discard，不能转移到替换实例。take 最多保留 32768
个合并后的区间，元数据、target 和 spans 不可变。source() 仍受既有 AST 深度/节点预算限制。

```ts
const recorder = await session.recordVst3Automation(target, { mode: "touch", parameterIds: [0] });
// Play and edit the vendor parameter. Stop recording before pausing transport.
const take = await recorder.stop();
const replacement = take.source(0, originalSource);
// Explicitly replace the owning global Project-beat lane's source, then session.update().
```

stream control 1 增加 startEdits/readEdits/stopEdits/discardEdits 显式命令；其他命令应答不
增加字段，旧 helper 对新命令拒绝。startEdits 创建本 helper 内唯一 captureId；重复启动
未结束日志拒绝。read/stop 带 captureId/fromSequence，游标确认前面已收取的数据，最多返回
256 个连续事件；同一游标可重试，倒退或越过已产生数据均拒绝。每次日志最多保留 4096 个
未确认事件与 4096 个待定位事件，达到预算或 vendor 队列丢失事件时整次日志标为 failed，
不能把部分结果当成功录制。discard 显式取消；换图后实例 generation 验证拒绝旧日志。

`nextSequence` 是服务端累计事件数量，分页续读必须用 `firstSequence + events.length`，
不能直接用 nextSequence 跳过尚未读取的页。游标过期或 captureId 不匹配返回
PluginTaskConflict，音频实例继续运行。失败页只提供错误，客户端必须丢弃此前积累的数据。

事件保留 begin/value/end、ParamID 和归一化值，附实际下一音频块的 sequence、Transport
与 reset 标记。VST3 编辑回调没有 sample offset，本宿主采用下一处理块首帧量化，不用
Node 墙钟或 UI playhead 猜测位置；零样本 flush 不推进日志时间。暂停/停止期间 pending
不能被虚构为已录制事件。stopEdits 返回 stopping，下一真实音频块定位已接收事件并写入
endPosition 后成为 stopped；后续新手势不混入。没有下一块时可显式 discard，不假报完成。
纯观察 startEdits 日志不自动写源码，也不更改既有 automation 优先级。
上述 startRecording 另提供 Touch/Write 覆盖与 SDK 区间合并；DAW 录制与源码事务见本文开头。

`node scripts/smoke-vst3-edits.mjs` 使用真实 IComponentHandler 回调、精确原生音频包与
SDK/N-API 模拟输出，验证分页、暂停、seek、停止、丢失和换图失效；Recording Gain 类独立
检查实际参数样本偏移、Touch/Write 覆盖与释放。SDK take 的 Source 与独立手写阶梯曲线
逐字节 WAV 对拍，并验证 Save/reopen；主限幅器延迟与 FIR 过渡显式计入增益比较。
不打开系统音频设备。
此测试的回调由测试插件控制器主动触发，不作为厂商窗口鼠标录制验收。

## DAW 播放实例窗口与状态接受

实例面板的 Open native editor / Close native editor / Use current state 分别发送 Document
controlInstance(openEditor/closeEditor) 与 captureInstance。Node 先验证单实例 source site、
revision、读集与精确 class/hash，再通过独立 Preview IPC 连接查询实际播放图的 instanceId/
graphGeneration，执行原生控制。旧的 editInstance 独立静音克隆入口删除；工作台 catalog 的
独立配置 editor 保留。没有 Preview runtime 的文档服务明确报 PluginHostUnavailable。

Preview 新增 vst3Instances/vst3Control 帧与同名应答，携带 snapshotRevision（accepted
document revision + 1）与 instance control 1 数据。query 必须匹配当前已接受快照；
control 执行前后核对快照 revision 与图 generation。厂商调用在有界后台任务中执行，
控制线程继续接收 transport/换图，专用连接不消费 GPUI 的 Document reverse requests。
图尚未发布、已退役、版本不符或当前图缺少该实例时拒绝，不启动备用克隆实例。
后台任务最多同时 8 个，超过返回 BudgetExceeded。控制时限从原生 IPC 收到完整命令时
开始，包含控制线程排队和后台线程调度；执行前扣除已耗时间，已过期请求不调用厂商。

打开/关闭窗口不产生源码 revision，窗口关闭不回滚已听到的修改。Use current state 捕获
当前 processor 状态，复核 class/hash、source reads/generation/revision 后生成原有配置
wrapper 并执行完整候选验证；成功形成一条可 Undo/Redo/Save 的文档事务。失败保留源码与
最后接受的工程。此入口只允许可隔离的一个配置使用，不猜测共享定义应该修改哪些实例。
厂商窗口仍属于 helper 主线程，慢 UI 调用可阻塞该 helper 的音频；不声称 UI/DSP 并行。

`node scripts/smoke-vst3-configuration.mjs` 包含 source-built Dense 插件的真实 Preview/
DawRunner 捕获、Undo/Redo/Save/reopen 回归，不打开音频设备。真实 VestiGain 可运行：

```sh
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3-live-daw.mjs --editor
```

该检查使用 headless Preview、模拟 sink 与实际原生厂商窗口，验证两次独立 addEffect 使用、
改参不改源码、捕获后准确写回一个实例、相同状态重复接受不增加历史、旧 generation 拒绝，
以及保存重开后的逐样本 WAV 增益。程序化窗口开关不能代替可视点击、手势或缩放验收。
同一 owner 的共享数组值没有可隔离 source boundary 时仍拒绝；可通过独立 addEffect 使用隔离。

## 工程实例控制（instance control 1）

`Session.vst3Instances()` 返回当前已接受图的 graphGeneration（canonical u64 十进制字符串）、
prepared/active/retired 状态及按稳定 instanceId 排序的 VST3 实例目录。目录覆盖 Channel
乐器、Channel insert 与 mixer/Master insert；同一插件的多个实例保持独立，不按链索引寻址。
generation 每次编译唯一，源码 revision 相同也不复用。无 instanceId 的旧快照不生成临时身份。

`Session.controlVst3Instance({graphGeneration, instanceId}, command, {timeoutMs?})` 通过异步
N-API 后台任务执行 stream control 1，默认 5 秒，上限 10 分钟。返回同一 target 和最新 state；
capture 含配置。命令前后核对图 generation 和 active 状态，旧图/尚未发布的图报
PluginTaskConflict，未知或不支持的实例明确拒绝。编译失败保留原图和控制目录；删除、重排
或替换后不能把旧 target 转绑到新实例。控制修改只作用于播放实例，不修改 authoring 数据。
保存和离线导出仍以最后接受的源码快照为准，需要显式通过配置事务接受捕获结果。

```ts
const session = await project.compile();
const inventory = session.vst3Instances();
const instance = inventory.instances[0];
if (inventory.state === "active" && instance) {
  const target = { graphGeneration: inventory.graphGeneration, instanceId: instance.instanceId };
  await session.controlVst3Instance(target, { kind: "openEditor" });
  // Close detaches the vendor window; edited values remain on this processing instance.
  await session.controlVst3Instance(target, { kind: "closeEditor" });
  const captured = await session.controlVst3Instance(target, { kind: "capture" });
  // Explicitly apply captured.state.info.configuration through the owning authoring boundary.
}
```

控制 capability 由 Rust PluginInstance 提供，编译时收集只读目录，设备 callback 不遍历
字符串或执行控制。换图处理边界只用原子状态激活新目录、退役旧目录；进程和窗口销毁仍由
原回收线程处理。厂商执行前释放 engine registry mutex；完成后再核对该 engine 的当前图。
浏览器原生 facade 对这些控制明确返回 PluginCapabilityUnsupported。

初始化参数由 VST3 adapter 入队，Channel/insert/Master 不在首块重复播种；播放前实时
修改因此保持有效，时间线自动化仍按其帧位置生效。helper 在发布 Ready 前零样本排空
初始化队列，避免占用首个 PCM 自动化的容量；setParameter 也先排空已有事件，再入队新值，
防止完整 4096 参数队列丢失第一条实时修改。request JSON 上限
16 KiB；N-API 后台任务开始执行时启动控制超时，返回 JS Promise 前再次核对当前图。

## 常驻实例控制（stream 11 / control 1）

原生 `Session::controller()` 返回只供控制线程使用的可克隆句柄，绑定该 Session 的唯一
helper 生命周期。`openEditor` / `closeEditor` / `poll` / `setParameter` / `capture` 经容量
8 的队列进入原有 IO worker，与音频请求串行写入私有 socket；控制帧前缀为 `OXVC`，
随后是有长度上限的 control 1 JSON。音频仍用原有 40 字节包头和多总线 PCM；stream 1…10
及未来版本拒绝。控制不进入 RealtimePort，也不调用 JavaScript。

helper 主线程在音频块之间和空闲时泵送 AppKit，所有厂商调用串行。live 窗口只提供 Close，
关闭仅 detach view，处理实例与当前参数继续存活；没有虚假的 Cancel 回滚。setParameter
验证实际 ParamID/可写性，并通过零样本 process 刷入同一个 DSP；capture 返回该实例的
完整 Vst3Info/state。nextSequence 标记控制发生在第几个音频请求之前，不表示设备播放头。
capture 在主线程直接 getState，不调用 setProcessing(false/true)，保留已发声音符与 tail；
符合 [VST3 processing/state 约定](https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Processing.html)。
调用失败保留稳定错误；超时中止整个 Session，避免已超时的编辑稍后偷偷生效。关闭 Session
立即中断 socket 并回收进程，旧句柄不能操作重建后的实例。

厂商窗口/状态调用仍可能阻塞 helper，受控制总截止时间与原音频超时保护；这不是独立 UI/DSP
并行执行的保证。运行中 I/O/latency/参数表改变使旧布局失效，不在旧 PDC 图内重协商。
stream 11 在响应 flags 的 bit 3 标记 restartRequired；请求不得设置，标记响应的 PCM 必须全零。
helper 在下一次 process 前检查控制通知，并在每次 process 后复查；发现变化就关闭编辑器、
捕获当前状态并停止处理。触发变化的整块及后续块保持原 Ready 形状、序号和时钟，只返回静音。
旧 helper 和控制句柄保留，poll/capture 的 state.restart 返回唯一原因集合
（io/latency/parameters/reload）、新 latencyFrames/tailFrames；capture.info 返回冻结的当前
状态及参数/总线描述，不重置为源码中的旧配置。SetParameter/OpenEditor/新录制返回
PluginRestartRequired；旧录制明确失败，仍可读取失败页和取消，不产出跨布局的录制片段。
恢复必须在控制侧用捕获状态准备新实例、验证路由和参数、重算 PDC 后换图；旧图上的 Seek/Play
不得隐式销毁冻结状态。通过捕获配置的显式事务恢复，不自动接受厂商窗口修改。
此原生控制面不自动写回工程源码；DAW 通过上述 captureInstance 接受状态，gesture
录制使用本文开头所述的独立时间坐标和自动化事务。

## Channel MIDI 输出（Engine 1.7 / stream 11）

```ts
const source = project.addChannel({ instrument: vst3Config(generator) });
const destination = project.addChannel({ instrument: vst3Config(synth) });
source.routeMidi(source.instrumentInstance, [destination]);
// Channel insert 的 MIDI 输出同样可连接；空数组断开。
source.routeMidi(effectInstance, [destination]);
```

连接属于 Channel 快照 `midiRoutes: { [instanceId]: channelId[] }`，不属于可复用 PluginConfig。
源必须是该 Channel 的乐器或 insert；目标必须声明原生 MIDI 输入。拓扑校验拒绝自连、环、
重复目标和悬空实例。一个事件可扇出到多个目标，各目标只收到一份；多个源按稳定拓扑和
实例链顺序合并。目的地再按 sample offset、note-off → parameter/controller → note-on 排序。
删除源实例清理其路由，effect 重排保留连接；保存/恢复、源码配置编辑与 Undo/Redo 保留实例身份。
Preview 显示并可导航连接，路由编辑通过源码 API。

inspect/Ready 增加 `noteOutput`。初始化 `options.midiOutput` 默认 false，启用时显式激活
主事件输出 bus 0，要求 noteOutput。每个回复最多 256 个 kind 4/5 消息；原始端口用
`output_events()` 借用最近一次成功接收的消息，下次 receive/故障清空，设备 callback 不访问插件。
只提供 PCM 调度的 ScheduledPort/Manager 在 prepare 拒绝 midiOutput=true，不能静默消费事件。
wire `type: "midi", frame, message: [status, data1, data2]` 仅允许 MIDI 1.0 通道消息；
Program Change/Channel Pressure 的第三字节必须零。kind 4 的 ParamID/f64 保留字均零，
packed MIDI 按低到高字节保存这三个字节，最高字节零。回复禁止 parameter/note 专用旧输入记录。
note velocity/pressure 从 VST3 normalized 值舍入为 7-bit；channel 和 segment offset 保留。
控制器、弯音与压力按接收插件 IMidiMapping 进入参数队列，未映射控制器由 VST3 语义忽略；
Program Change 使用 loader 的 root-unit program 参数映射，保留 sample offset。
实例收到可能映射参数的 MIDI 后，宿主不再用上次 authored 值消除参数事件，确保 MIDI 改值后
相同数值的显式控制/自动化仍能送达；同段 A→CC→A 也保留最后一次 A。
每个实例的参数、乐谱与路由输入合计最多 256，输出最多 256；loader 的队列满、原始事件
拒收、无效位置/值均终止当前段，不丢弃 note-off 继续播放。restart 响应清空 PCM 和事件。

当前只支持主事件总线的 NoteOn/Off、Poly Pressure、CC、Program Change、Channel Pressure、
Pitch Bend 与下述 SysEx；非零 note tuning、Note Expression 和其他事件在开启捕获时明确拒绝。
关闭捕获的普通音频实例丢弃未路由输出。Mixer/Master 反馈 MIDI、内置/C ABI 1 接收、
硬件 MIDI 与插件运行时 SMF 录制仍不支持。音频 PDC 不改 MIDI 时间，音频 mute/solo/fader/
mix/bypass 不阻断事件；Channel insert 不会自动收到乐谱输入。
MIDI-only 插件保留空的 audioBuses.inputs/outputs 与 inputChannels/outputChannels=0，
必须提供主 MIDI 输出。无论厂商标记为 Instrument 或 Fx，它在工程中均作为 Channel
instrument 节点使用，经 routeMidi 连接下游音源；本节点的乐器音频恒为零，不伪造物理音频 bus。
有 MIDI 输入者可处理乐谱或上游 MIDI；无 MIDI 输入的自主生成器随播放时钟运行，绑定有效
乐谱时 compile 明确拒绝，路由输入也在能力校验时拒绝。配置、预设、当前状态捕获与保存恢复
沿用相同生命周期。零输出但有音频输入的分析器不属于此能力，仍拒绝。
独立 stereo WAV API 和只消费 PCM 的 ScheduledPort/Manager 拒绝零音频输出；使用工程图
把 MIDI 连到音源后导出，不能用静音 WAV 冒充 MIDI 生成结果。

验证：`node scripts/smoke-vst3-midi.mjs` 使用真实原生 fixture，分别进行 400 次音频/MIDI 与
400 次纯 MIDI port 往返，再对照两级 MIDI-only 链、自主生成器的离线 PCM 和模拟 sink。
包含故障注入、reset、配置恢复、运行中捕获、无输入能力拒绝、保存/恢复与源码事务；
不打开硬件音频输出。

## 主总线 SysEx（stream 11）

`renderVst3Wav` 接受 `{ type: "sysEx", frame, data: number[] }`。完整消息必须以 F0 开始、
F7 结束，中间字节为 7-bit data；长度 2…4096 bytes，不接受拆分片段或嵌入 realtime status。
每个 process 段的输入/输出 SysEx 各最多 16384 bytes，与普通 MIDI/参数/音符合计最多 256
事件。离线 API 在加载插件和创建 WAV 前校验每块预算；tail 区间仍不可放置输入事件。
SysEx 与控制器同一排序优先级，保留实际 sample offset 和同帧顺序。

40-byte stream 头 offset 36 为 u32 payloadBytes；PCM 和 24-byte 事件数组之后紧跟 payload。
kind 5 事件的两个 u32 字段分别存 payload offset/length，末尾 f64 位必须全零。双向校验
范围、消息格式、长度和总预算；旧 stream 1…10 拒绝，Engine 1.7、control 1 与 C ABI 1 不变。
Rust `Event` 使用 `MidiBytes` 引用；`BlockContext.payload` 提交借用，`output_payload()` 与
`output_events()` 一起消费，均仅在下次 receive 前有效。预分配 payload 不扩大普通事件记录，
submit/receive 与 ScheduledPort 的短段拼包不分配/释放，不锁、不阻塞。

图路由为每个目标复制到预分配 arena，扇入先检查事件与字节预算，溢出不部分接受；扇出保留
独立范围。SysEx 可能改变参数，因此会禁用以旧 authored 值为依据的参数去重。seek/reset、
restart 和故障清除旧消息与 payload。helper 从插件 outputData 复制字节后才跨进程发送，
不持有厂商指针。错误或超预算输出终止当前插件，不截断后继续运行；未启用捕获的实例仍忽略
未路由输出。ScheduledPort 支持 SysEx 输入，仍拒绝 MIDI 输出捕获。

`node scripts/smoke-vst3-midi.mjs` 包含满载 SysEx echo、输出指针失效后的数据、格式/预算
故障、MIDI-only 透传到音源、扇出、mute、保存恢复与模拟 loop/seek。端口和短段拼包有零
allocation/free 测试。详细证据见 [接入记录](../reports/2026-09-21-vst3-sysex.md)。

## Mixer insert 多路输入与辅助输出（Engine 1.6）

MixerChannelSpec.insertRoutes 按稳定 effect instanceId 保存 `{inputs?, outputs?}`，两个映射
的键均为物理辅助 bus 索引 1…15，值为 MixerChannel ID。bus 0 保留串行主输入/输出。
TS 使用 `bus.routeInsert(instance, routing)` 完整替换该实例的路由，省略 routing 则删除；
`bus.insertRoutes` 返回防御性副本。路由不属于可复用 PluginConfig，重排保留、删除实例移除。

输入从指定 mixer bus 的 post-fader 输出取得；输出从指定 insert 的湿辅助总线取得，
不经过后续 inserts，受该 insert 的 mix/bypass 和所属 bus 的 level/balance/mute/solo 控制，
不乘所属 bus 的 masterSendRatio。辅助输出先补偿后续 inserts 的延迟，与所属 bus 主输出
处于相同时间坐标，再执行到目的 bus 的 PDC。显式输入对齐到所属 bus 入口，再补偿此前
inserts 的延迟；既有 broadcast sidechain 也补偿此前 inserts 的延迟。显式 bus 1 输入
覆盖该实例的 broadcast sidechain，其他实例不变。没有路由的辅助 bus 在 prepare 停用。

输入/输出连接与普通 send/sidechain 共同参与 DAG 与全图 PDC。主输入的 Channel 直达
贡献也按目的 bus 的入口延迟补偿。拒绝未知/非规范索引、悬空实例/目标、非本 owner 实例、
所有反馈环、以 Master 为源和从 Master 向外输出。Master inserts 可接收其他 bus 输入。
能力按恢复配置后的实例验证，不能将同类插件另一个实例的布局用于此实例。
低于 Engine 1.6 的快照携带 insertRoutes 时拒绝，不改写恢复文件。

本接口用于 Mixer inserts；Channel 的上游乐器/insert 链先于 Mixer 执行，不接收 Mixer
回送。需要多路输入或效果辅助输出时，将效果放在对应 Mixer bus。C ABI 1 不增加能力。
音频设备 callback 仍只读取完成 PCM；连接准备、激活和 PDC 缓冲均在编译阶段完成。

Mixer stem 的 tap 包括实例直接送往 Master 的辅助输出，位于 Master inserts/fader/limiter
之前；其他辅助输出计入下游 bus 的 stem。导出范围见 [06](06-format-and-export.md)。
`smoke-vst3-buses.mjs` 覆盖真实 3 输入/3 输出效果的逐样本计算、激活、保存重开、stems、
模拟换图与无效路由拒绝，并验证配置源码事务的 Undo/Redo/Save 保留实例路由。

```ts
const effect = fxBus.addEffect(vst3Config(registration));
fxBus.routeInsert(effect, {
  inputs: { "1": drums.id, "2": vocals.id },
  outputs: { "1": parallel.id, "2": ambience.id },
});
```

## 多总线传输（stream 11，保留 stream 7 的总线布局）

### 工程乐器多输出（Engine 1.4）

Channel.outputRoutes 是可选的 `{ "1": mixerBusId, "2": mixerBusId }` 映射，键为插件物理
输出 bus 索引 1…15。输出 0 继续经 Channel.effectChain 和 mixerChannelId；辅助输出
绕过 Channel inserts，经同一 Channel level/pan/mute/solo 后进入各自目标 mixer bus。
各路有独立预分配 PDC，与最长 Channel 链对齐，再参与原 mixer DAG、send/sidechain 和 stems。
同一乐器实例每段只 process 一次；事件、state 和时钟不复制。求和按 Channel ID、物理 bus
索引升序固定。未路由辅助输出在 VST3 prepare 显式停用，不生成后丢弃；修改路由需重新 compile。
未知索引、内置/C ABI v1 的辅助输出、悬空目标与 Engine <1.4 携带该字段均拒绝，恢复数据不改写。
乐器辅助音频输入当前停用；Mixer 效果的多路输入输出使用上文 Engine 1.6 契约。

```ts
const drums = await registerVst3Plugin(project, source);
const kick = project.addMixerChannel({ name: "Kick" });
const snare = project.addMixerChannel({ name: "Snare" });
const channel = project.addChannel({
  instrument: vst3Config(drums),
  outputRoutes: { "1": kick.id, "2": snare.id },
});
// Output 0 retains channel.mixerChannelId. Omitted auxiliary outputs are inactive.
```

inspection 和 Ready 的 audioBuses 按物理 VST3 索引保留每个输入/输出 bus 的 channels、active。
每个方向最多 16 个 mono/stereo bus；surround 仍拒绝。inputChannels/outputChannels 表示
第 0 个 bus，不是所有 bus 的通道总和。stream options.busActivation 可在初始化、恢复 state 后
显式激活或停用每个 bus，数组长度必须匹配实际布局。缺省保留插件声明的默认激活状态。

stream 11 包头为 40 字节：在 stream 5 的 32 字节之后增加 busCount 与 payloadBytes 两个 u32。
PCM 按 bus 索引排列，每个 bus 用两个 planar float32 声道，mono 输入取均值、输出复制。
无输入插件仍使用一个全零占位 bus；inactive 输入要求全零，inactive 输出始终全零。
请求/回复分别携带输入/输出方向的完整总线数，不要求两者相等；IO 层核对回复总线数。
stream 11 允许 MIDI-only 回复 busCount=0，不携带 PCM，仍保留 frames、transport、reset
和有界 MIDI 事件。请求继续使用一条全零输入占位以声明帧数；receive_buses 接收空输出
数组，双声道 receive 拒绝并清零传入缓冲。旧 stream 9 不具备该回复语义。
旧 stream 1…10 与未知版本拒绝，不迁移持久化数据。队列按已验证的最大方向总线数预分配。
RealtimePort.submit_buses/receive_buses 是完整总线接口；双声道接口不接受多个物理总线。
所有端口操作仍无分配、锁和系统调用。工程乐器辅助输出路由使用上述 Engine 1.4 契约。

工程 bus insert 可将现有 mixer sidechain 送到 VST3 输入 bus 1，也可按实例显式选择
多个辅助输入和输出。显式 input 1 覆盖该实例的广播侧链；未连接辅助总线在 prepare
停用。独立 stereo WAV API 继续要求单个输出及最多一个输入，不静默丢弃额外总线。

提供可选的 `@oxitone/vst3` 包和一个本地 Rust helper。它只在显式调用
`oxitone/vst3` 或 `@oxitone/vst3` 时检查指定的 `.vst3` bundle，并在单独的 native 进程中
完成 inspection、离线 WAV 或常驻工程处理。主入口不会自动扫描、联网或下载。
显式 registerVst3Plugin 后可用为 Project 乐器/insert；DAW 通过源码事务分配。工作台
同时保留离线 WAV、预设文件及渲染音频导入。

## 边界

- helper 只支持 macOS，路径必须是本地绝对路径；不会执行 shell。npm 安装通过 optionalDependencies
  选择 `@oxitone/vst3-host-darwin-arm64` 或 `@oxitone/vst3-host-darwin-x64`，运行时不下载或编译。
  SDK 优先使用每次调用的 hostPath，其次 `OXITONE_VST3_HOST_PATH`，再寻找匹配架构的安装包。仓库
  开发时可运行 `cargo build -p oxitone-vst3-host --features host,stream`。
  显式覆盖路径错误时不回退；普通消费工程不搜索 cwd/PATH。平台包构建使用锁定的 host+stream，
  prepack 核对 Mach-O 架构与执行权限；CI 每个 macOS 架构打包并在仓库外验证 SDK 自动定位及启动。
  `scripts/smoke-vst3-package.mjs` 保留审查 tarball 到 target/ci；此开发工作不发布 npm 包。
- 每次 inspection/render/editor 都启动独立 helper。Node 进程不加载 VST3 动态库；helper 的 stdout
  和 stderr 不属于协议，结果通过专用管道返回。超时、取消、退出码异常和超过 8 MiB 的响应
  都会终止进程并映射为稳定错误。
- `bundlePath`、class ID、可选 SHA-256 和签名策略会在 native 边界再次校验。bundle 拒绝
  symbolic link，最多 65,536 个条目、深度 64、总大小 1 GiB；`signed-only` 要求 Apple
  签名，`any` 只适合本地开发。
- class ID 是精确的 32 位十六进制音频 class 标识。依赖的 loader 使用 moduleinfo/factory
  发现 class；metadata 未声明请求 class 时可回退到默认加载，但始终核对 factory 的实际 UID。
  不接受 retired UID 替换或另一个 class。加载前后复核 bundle hash；这不是恶意原生代码沙箱，
  也不声称文件检查与动态加载构成原子快照。
- inspection 会返回参数、通道数和不透明 configuration state。state 最大 4 MiB，保留时必须
  同时保留 class ID 与 bundle hash；格式版本不认识时拒绝恢复，不改写原始数据。
  Rust native boundary 会再次检查 configuration 的 class/hash 字符串、canonical base64 和
  解码后 4 MiB 上限；这些校验不依赖 TypeScript，坏 state 在加载插件前返回
  `PluginConfigInvalid` 或 `BudgetExceeded`。
- render 使用离线模式、显式 sample rate/block size、参数事件、MIDI note 与 SysEx 事件，输出新的
  stereo float32 WAV。输入 WAV 只接受同采样率的 mono/stereo 文件；事件必须位于 content
  区间，tail 区间只用于继续处理。目标文件已存在时不会覆盖。
- 独立 stereo WAV API 要求零/单个 mono/stereo 输入 bus 和单个 mono/stereo 输出 bus，
  多总线使用上文 stream 11；surround 显式拒绝。mono WAV 输入复制到 stereo；stereo 到 mono 取均值；mono
  输出复制为 stereo WAV。输入最大 1 GiB，解码最多 64 Mi 个 float samples（256 MiB）。
- 帧事件先按 frame 排序，同帧按 noteOff、parameter、noteOn 稳定排序；参数必须已声明且
  可写，自动化必须 canAutomate。note 必须有 note input，velocity 经当前 MIDI facade 量化为
  7 bit。独立 renderVst3Wav 的 tempo/timeSignature 固定；工程执行器另提供 tempo map、loop/seek 映射。
- 独立 renderVst3Wav 输出保留插件 latency，不做 PDC/自动裁头；工程执行器提供图 PDC。
  tailFrames 是调用方要求的零输入处理长度，
  report.tailFrames 是插件报告值，不据此自动延长或保证混响尾音已结束。

## API 形状

`scanVst3Bundles({directories?,signal?})` 显式列出用户/系统 VST3 目录或指定绝对目录，
不加载插件；跳过 symlink/隐藏目录，不进入 bundle 内部。最多 32 个根、16 层、65536 个条目、
4096 个 bundle，超过预算报错。`listVst3Classes({bundlePath,expectedHash?,allowPlugins?},host?)`
在独立 helper 内验证签名与双重 hash 后枚举真实 factory 音频 class，返回名称、classId、
版本和厂商；上游详细枚举可初始化首个 component，受整进程取消/超时限制。枚举结果不等于
各 class 已通过工程 I/O 能力验收；分配前仍执行 inspection 与 native registration。

```ts
import { inspectVst3Plugin, renderVst3Wav } from "oxitone/vst3";

const source = {
  bundlePath: "/Library/Audio/Plug-Ins/VST3/Vendor.vst3",
  classId: "56455354494741494e30303030303031",
  allowPlugins: "signed-only",
};

const info = await inspectVst3Plugin(source, { hostPath: "/path/to/oxitone-vst3-host" });
const report = await renderVst3Wav(
  { ...source, expectedHash: info.sha256 },
  {
    path: "/tmp/render.wav",
    inputPath: "/tmp/input-48000.wav", // 效果器输入；无音频输入的乐器省略
    frames: 48_000,
    configuration: info.configuration ?? undefined,
    parameters: { "0": 0.5 }, // 使用 inspection 返回的 ParamID
    events: [],
  },
  { hostPath: "/path/to/oxitone-vst3-host" },
);
```

`inspectVst3Plugin()` 和 `renderVst3Wav()` 都是控制侧/离线 API。它们不会把 VST3 processor
接入 Oxitone realtime graph，也不会打开 CoreAudio 设备或 VST3 editor。`configuration` 是
跨调用传递的 opaque state；预设文件把它和 plugin hash 一起保存。

```ts
import { saveVst3Preset, loadVst3Preset } from "oxitone/vst3";

if (!info.configuration) throw new Error("Plugin did not provide restorable state");
await saveVst3Preset("/tmp/gain.oxivst3.json", {
  formatVersion: 1,
  kind: "oxitone-vst3-preset",
  name: info.name,
  source: { bundlePath: source.bundlePath, classId: source.classId },
  configuration: { ...info.configuration, parameters: { ...info.configuration.parameters, "0": 0.5 } },
});
const preset = await loadVst3Preset("/tmp/gain.oxivst3.json");
// 后续 render 使用 preset.configuration；签名策略仍由调用方明确提供。
```

`parseVst3Preset(unknown)` / `loadVst3Preset(path)` 返回严格的 format 1 `Vst3Preset`；
`saveVst3Preset(path, preset)` 只发布新文件，绝不覆盖。预设要求本地绝对 bundle 路径，
source/configuration 的 class 一致，configuration 保留 SHA-256、opaque state 和归一化参数。
预设不能包含 `allowPlugins`，读写都不加载插件或改变工程信任策略。文件上限 8 MiB，加载拒绝
symlink、目录和特殊文件，按文件长度分配缓冲并拒绝读取期间长度变化；未知版本报
`ProtocolVersionUnsupported`，原始恢复数据不变。
保存先以 0600 权限写相邻暂存文件并 fsync，再 hard-link create-only 发布并同步目录；并发
保存同一目标只有一个成功，失败清理暂存文件，不删除已有目标。文件系统操作失败报
`AssetUnavailable`；发布后的目录 fsync 若失败，目标可能已存在，重试仍不会覆盖。

## 厂商原生配置编辑器

`configureVst3Plugin(source, options?, host?)` 在独立 helper 中恢复 configuration、应用
parameters 并返回重新检查的 `Vst3Info`。options 与 editor 相同；不创建窗口或音频设备。
参数 ID/可写性在恢复 opaque state 后检查，允许预设改变参数表；后续显式 parameters
覆盖预设参数。工作台重查已导入/已 Apply 的配置时走此路径，不用默认实例的参数表验证预设。
原始预设保留，配置失败不激活，也不重写恢复数据。helper 新增 `configure` operation，
仍使用 protocol 1；旧 helper 对未知 operation 明确拒绝，不产生默认配置的假成功。

配置会话使用完整物理总线表进行零样本处理，覆盖无输入、mono/stereo、多输出与 inactive
slot。每轮处理前后服务 restartComponent：I/O/latency 通过 stop/deactivate/重新协商/
reactivate，重建零样本总线缓冲，重新查询参数表和控制器值。每次 flush 最多 8 轮；反复
改变能力或要求 reloadComponent 时明确失败。Apply 前再 flush 并在停止处理后捕获 state，
保证最后一次 GUI 修改进入 processor。以上是独立配置阶段的协商，播放图仍维持编译时
能力/PDC，不把运行期布局变化偷偷应用到旧图。

预设与显式参数覆盖先按 ID 合并再入队；全部参数失效通知在前一次 flush 已清空队列后
重新同步，避免重复 4096 项参数占满队列后静默丢值。零样本调用的音频指针/总线数为空，
宿主仍按最新物理布局持有并验证缓冲表。验证使用拒绝任何非零音频处理的原生 fixture；
`node scripts/smoke-vst3-configuration.mjs --benchmark` 同时记录含 helper 启动的控制面耗时，
3 次预热、30 个样本。它不代表音频 callback 性能。

`editVst3Plugin(source, options?, host?)` 打开独立 helper 主线程拥有的 AppKit/IPlugView 窗口。
options 接受 sampleRate（默认 48000）、blockSize（默认 128）、configuration 和归一化
parameters。Apply 返回含新 configuration 的 `Vst3Info`；Cancel、Escape 或关闭窗口返回
null。默认交互超时 10 分钟，可用 host.timeoutMs 覆盖；AbortSignal 终止并回收 helper。
始终核对 class/hash 和完整 state；没有厂商 editor 则返回 PluginCapabilityUnsupported，
现有通用参数面板仍可用。未知协议拒绝；editor request/result 仍为 helper protocol 1。

窗口关闭前先 detach 厂商 view，保留父 NSView/回调直到清理完成。resize 遵守厂商约束与
IPlugFrame 请求，尺寸限 16…4096；macOS 使用 NSView 的原生点坐标/Retina backing。
AppKit event loop 和插件调用顺序执行，零样本 process 把 controller 修改刷到 processor；
不打开音频设备、不发送试听音符。该窗口编辑独立配置，Apply 后通过候选重新编译生效，
不提供对正在播放实例的实时 GUI gesture/automation 录制或边调边试听。

```ts
import { editVst3Plugin, vst3Config } from "oxitone/vst3";
const edited = await editVst3Plugin(source, { configuration: info.configuration ?? undefined });
if (edited?.configuration) {
  const updated = vst3Config(registration, { configuration: edited.configuration });
  // 将 updated 用于乐器/效果器配置，或保存为 Vst3Preset。
}
```

DAW 工作台的 Native editor 更新会话配置，refresh/re-inspect 后保留已 Apply 的状态。
已插入插件面板使用 controlInstance/captureInstance，连接播放实例并显式写回准确 source site，支持
Undo/Redo/Save；取消/超时/陈旧版本/预算失败保留原代码和图。回写规则见 [19](19-plugin-configuration-source.md)。

## 工程实例与执行域

```ts
import { Project } from "oxitone";
import { registerVst3Plugin, vst3Config } from "oxitone/vst3";

const project = new Project();
const registration = await registerVst3Plugin(project, source, { hostPath: "/path/to/oxitone-vst3-host" });
const config = vst3Config(registration, { configuration: info.configuration ?? undefined });
// 乐器使用 instrument: config；效果器使用 effectChain: [config] 或 bus.inserts。
project.addChannel({ effectChain: [config] });
await project.renderWav({ path: "/tmp/project.wav" });
```

低层 `Project.registerVst3` / native `registerVst3` 接受 registrationVersion 1、source、
helperPath 和 inspection metadata。native 在新 helper 中再次比对 class/hash、实际 I/O、
note input、参数 ID/可写/自动化 flags。身份为 `vst3.<lowercase-class>` 与 `0.0.0+<sha256>`，
不复用包自称的 C ABI ID/version。只注册 factory，不在 Node 或 Preview 进程加载 VST3 库。
工程保存只保存 snapshot/state；重开可移植 Project 后必须显式重新注册本地插件。
源码工程的 withVst3Registration 则保留选择，在每次求值与 Preview 编译时重新验证。

图有三个明确入口：process_block 维持 hard realtime；process_isolated_block 供后台
buffered worker 等待隔离结果；process_offline_block 供离线导出并设置 VST3 Offline 模式。
只有后两者能调用 process_isolated，回调只读取已完成的 ring PCM。独立 ScheduledPort
不是工程的执行器；图 PDC 使用插件固有 latency，不再叠加 SDK 固定调度延迟。

轨道音源、轨道 insert、bus/Master insert 共用原图事件/混音/干湿旁路/PDC 顺序。
每个处理段传递当前 Project tempo map、原始拍号、beat/bar、绝对工程帧、连续处理帧和
loop；音符与参数保留段内 frame offset。每段最多 256 个实际事件，超限返回 BudgetExceeded。
参数为归一化值；read-only 不出现在可写 descriptor，非自动化参数不能成为 automation 目标。
configuration 进入 Engine 1.3 的 InstrumentRef.state 或 EffectRef.state；未知版本拒绝而不重写。
VST3 不能声明 Oxitone sample `resources` 绑定；instrument/channel/bus 的非空 resources
在图验证时明确拒绝。插件自有资源由其 state/厂商机制管理，不静默丢弃宿主资源声明。

seek/loop 的实时安全 reset 标志与下一段位置、事件和音频一起提交；helper 在段首执行
标准 setProcessing(false/true)，保留当前参数和激活资源，不重启进程、不重新 setup。
这是 [VST3 规定的 DSP 重置流程](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Workflow%2BDiagrams/Audio%2BProcessor%2BCall%2BSequence.html)；
非合规插件不清空内部声音的行为不能由宿主保证。初次 prepare 后未处理过 PCM 的 seek
无需重置。helper timeout/crash 返回稳定错误、停止播放并清空当前输出；显式 seek/recompile
可重建故障进程，恢复最初 configuration 与最新参数，latency 变化要求重新 compile。
普通 C/builtin 图继续使用原实时执行能力。

stream 11 保留 initialization processingMode: realtime|offline（默认 realtime），在 binary
header direction 字的 bit 2 添加 processing reset，回复必须原样确认；其余未知位拒绝。
版本 1/2/3/4/5/6/7 与未来版本均拒绝；inspection/render 仍为 helper protocol 1，configuration/preset
仍为 format 1。导出最后一段只处理真实剩余帧数。

验证入口：`node scripts/smoke-vst3-project.mjs`（VestiGain 工程 PCM/模拟设备）与
`node scripts/smoke-vst3-daw.mjs`（VestiMIDISynth 源码分配/保存/恢复/MIDI 音符）。
分别要求 OXITONE_VST3_FIXTURE，后者还要求 OXITONE_VST3_INSTRUMENT；始终无硬件音频输出。

## DAW 操作与状态边界

Plugins → Add VST3：填写本地 bundle 路径，class ID 留空时 Discover plugins 使用隔离 helper
枚举所有音频 class 并 pin hash。明确填写 class ID 时仅添加目录条目，不执行插件。
选中 VST3 → Offline tools → Inspect：继承工程 `allowPlugins`（缺省 signed-only），独立
helper 获取真实参数和通道。此页面可编辑归一化初始参数、恢复 descriptor default、填写
输入/新输出 WAV、内容时长、tail、tempo；乐器增加测试音符。当前表单固定 48 kHz、128 帧、
4/4，SDK 可传其他受支持值。参数每页 32 个，不丢弃剩余参数；readonly 不可编辑。
Render WAV 使用 Inspect 捕获的 configuration、实际 hash 和参数覆盖。Cancel task 发送
`vst3.cancel`，绕过普通串行请求队列终止 helper，仍遵守 session/revision/requestId 幂等。
本地路径相对工程根解析；类/参数/输出错误由 Document Service/SDK/native 逐层拒绝。

Files / project 页提供 Preset JSON 路径、Save new preset、Load preset，以及 Add last render
to Playlist。Add VST3 表单也可直接 Load preset。加载仅恢复会话条目并 pin bundle hash，
仍须 Inspect 才能 render；Inspect 恢复 opaque state 后校验当前 class、hash 和可写参数，
显示实际恢复后的参数值和能力。保存包含 Inspect state 加当前参数覆盖，
不把单次渲染后的 DSP state 误当新预设。目标存在时须选择新路径。

Document 2.0 增加 `operation: {kind:"vst3", command:{kind:..., ...}}`，支持
`add` / `addBundle` / `remove` / `render` / `cancel` / `loadPreset` / `savePreset` / `attachRender`。
复用 `verifyPlugin` 实现 Inspect，`refreshPlugins` 重建目录；除 `attachRender` 外，这些操作
不创建音乐 revision、不写 `.ts` 或 Undo history。预设文件是独立的显式文件写入。
关闭工程后本地目录条目消失，预设可重新加载；此工作台不是持久化的实时插件实例。
普通浏览无动态库执行，Inspect 明确加载插件。最多 32 个本地条目，检查结果投影合计 8 MiB，
已检查与导入的 opaque configuration base64 合计 24 MiB；configuration 不广播到 GPUI。
刷新清理所有验证状态与临时参数覆盖，保留本地路径/class、已加载预设及其 hash。
文件变更后的 render 因 hash 不符失败；未 pin 的条目可重新 Inspect，预设 pin 不会自动更新。
Inspect 失败清空活动配置，保留导入预设用于诊断/恢复。工程失效/关闭中止任务，过期完成不能
回写新 view；已经发布的独立预设不因随后工程失效而删除。

`attachRender` 要求最近一次成功的 WAV，先由 Rust cacheSample 重新读取/解码并核对 report
的源 hash、帧数、采样率和 stereo 通道。拒绝超过 256 MiB 的文件和 symlink；工程
`assets/vst3` 逐级拒绝 symlink，按 PCM 内容 hash 保留自有 WAV。源码只记录相对资源 URI
和完整 SampleRef，不依赖原始输出文件、VST3 helper 或插件安装。随后经普通 Document 事务
追加 `Project.importAudio({name,startBeat,sample})`，新建 Track 和 sample clip；候选求值、
native 校验、accepted revision、Undo/Redo、journal 和 Save 均沿用现行源码链路。
startBeat 为非负工程 beat；tempoSync 为 off，不设置 durationBeats，保留全部内容和显式 tail，
也保留原有 latency，不做 PDC/自动裁头。重复导入创建独立轨道，资源可复用同一 hash。
Undo 或候选失败保留内容寻址资产，保证 Redo/恢复可用；本阶段没有自动清理未引用资产。

目录 source 为 `vst3`，使用独立 registration，可经 Assign 分配到匹配的音源/insert。
关闭工作台只隐藏视图；取消任务用 Cancel task。直接 SDK 超时默认 30 秒，可显式到 10 分钟；
工作台使用默认超时，长任务失败不会发布半个 WAV。

## 验证

默认测试覆盖 schema、无 helper、响应/超时/崩溃/取消、输出 create-only 和 cleanup、静态 npm
发现、会话预算与刷新、取消队列、表单范围、预设版本/预算/原子发布，以及源码音轨导入的
Undo/Redo/Save、输出篡改和原输出删除后重开。原生 feature 另测 WAV 通道转换与非法样本。
安装本地 VestiGain 后，显式执行（绝不查找或下载插件）：

```sh
cargo build --release -p oxitone-vst3-host --features host
pnpm --filter @oxitone/vst3 build
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3.mjs
# GUI：先 build CLI 和 pnpm build:preview --debug，再给上面命令加 --gui
# Benchmark：加 --benchmark；OXITONE_VST3_BENCH_OUTPUT 指定归档 JSON 路径
```

真实 fixture 验证 configuration 恢复、mono 有声输入、129 帧块内边界后 7 帧静音、hash/class/state
拒绝和不覆盖。GUI smoke 用实际字段/按钮 hit test 与键盘输入，写 WAV 后复核 PCM，确认源码
与 revision 在纯工作台操作时不变、refresh 失效；再验证预设保存/加载/恢复渲染、音轨导入、
快捷键 Undo/Redo/Save 和新进程重开。只用 simulated sink；不是听音、实时宿主或其他厂商兼容认证。

## 常驻原生音频通道（SDK 底层）

`oxitone-vst3-host` 的可选 `stream` feature 提供 Rust `stream::Session` 与 `RealtimePort`，
控制初始化由 `Vst3StreamStart` / `vst3-stream-start.schema.json` 描述，独立 stream protocol 11。
TS 只声明初始化契约；没有 JS PCM 循环。工程图通过下述独立后台执行器使用原生端口。
现有 `vst3-host` 依赖的 process 路径含 Mutex 和 controller 调用，因此它只运行在常驻 helper
主线程，不能直接适配为现行 graph 的同步 `PluginInstance`。

```sh
cargo build --release -p oxitone-vst3-host --features host,stream \
  --bin oxitone-vst3-host --example vst3-stream-probe
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3.mjs --stream
```

Rust 调用方式是控制线程 `Session::spawn(absolute_helper_path, start, options)`，返回一个
生命周期控制句柄和独占 `RealtimePort`。helper 必须显式构建 `host,stream`；客户端可只链接
`stream`，不依赖 VST3 loader。使用 `serde_json::from_slice::<stream_wire::Start>` 读取经过
schema 规范化的控制对象；options 指定 sampleRate、blockSize、configuration、parameters、
tempo 和 timeSignature，source 沿用精确 class/hash/签名策略与加载前后 hash 校验。

- `Session::spawn` 是同步控制操作，有单独的启动 deadline（默认 30 秒，1 ms…600 秒）。
  启动回复必须版本、身份和能力一致，参数去重；不认识的字段/版本均拒绝。初始化失败杀死并
  回收 helper。每次会话使用私有 Unix socket，fd 3 为双向协议；插件 stdout/stderr 不参与协议。
  握手后还必须在同一启动 deadline 内等到 IO worker 完成调度与等待器初始化，才交出端口。
- 控制期预分配 2…16 个音频块（默认 4），每块每总线最多 4096 帧 stereo f32，总线数按实际布局分配，另有 256 个事件和 16384 bytes payload。
  `submit(left,right,events)` 校验后复制进有界队列，返回从 0 开始的连续 sequence。
  无空闲块返回 `Full`，无部分接受；调用方必须重试或终止会话，不能静默跳块并继续旧时间轴。
  事件用块内帧偏移，参数必须已声明、可写且可自动化；MIDI note 通道/音高/速度同离线契约。
  无输入 bus 的乐器必须传入全零 input；mono 输入取两声道均值，mono 输出复制为 stereo。
- `receive(left,right)` 要求两个输出 slice 都为最大 block 长度，返回 sequence/实际帧数。
  暂无结果时清零并返回 None；短块余下空间清零。错误长度只清除最多 maxBlock 帧并返回
  InvalidBlock。`submit`/`receive`/`status` 不分配、不释放、不用锁、不做 I/O/时钟/进程调用。
  PCM、event 和队列存储全部预分配；10,000 次往返及队列满/故障路径有分配器计数测试。
- 后台 IO worker 负责 little-endian 二进制帧、socket 与 watchdog。每块单独的总 deadline
  默认 100 ms（1 ms…10 秒），覆盖写入与完整响应，缓慢滴流不能反复延长 deadline。
  响应必须有相同 sequence/帧数、合法协议头与有限 PCM；乱序或 NaN/Inf 不发布到完成队列。
  超时、进程退出、非法响应和插件处理故障分别 latch 为 TimedOut/Crashed/InvalidResponse/
  PluginFault；状态不能被 close 清掉。故障后 receive 清零，不重放已排队结果。
  helper 完成插件初始化后、IO worker 发布就绪前分别申请 macOS time-constraint 策略，
  period/constraint 为一个 block 周期、computation 为半周期；申请失败仍可运行并明确报告。
  IO 空队列用 Mach absolute wait 等待 100 µs，空闲进程退出检查间隔为 10 ms；在途 EOF
  立即处理。音频端不负责唤醒线程，不增加 syscall、时钟读取、锁或等待。
  初始化是 u32 LE 长度加不超过 8 MiB 的 JSON。音频帧头为 40 字节：`OXVB`、u32 版本、
  u64 sequence、u32 frames、u32 eventCount、u32 direction/flags（低位 1 请求/2 响应，bit 2 reset）、u32 processingMicros、u32 busCount、u32 payloadBytes；
  后接 72 字节 transport record，再接按 bus 索引排列的 left/right f32 LE 与 24 字节事件
  （kind、offset、ParamID、packed MIDI、f64 值），最后附 payloadBytes 个 SysEx 字节。transport 是九个 u64 LE word：flags、
  projectFrame、continuousFrame、projectBeat、barBeat、tempo、打包拍号、cycleStart、cycleEnd；
  beat/tempo/cycle 保存 f64 位表示，拍号为低 32 位 numerator／高 32 位 denominator。
  flags bit 0 表示存在、bit 1 playing、bit 2 cycle；缺省 record 必须全零，不认识的位拒绝。
  响应回显相同 transport record，IO worker 与 sequence/frames 一起校验；它不是插件反馈。
  请求的 processingMicros 必须为零；响应为 helper 处理墙钟时间（向上取整 µs，饱和 u32，
  包括线程被暂停的时间）。头、PCM 与事件合并为一次 write_all，读取先验头再整段读有界
  payload；内核短读/短写仍遵守整块 deadline。版本 1…10 或未知版本直接拒绝，不自动迁移。
  响应可以携带 kind 4 MIDI 或 kind 5 SysEx，restart 响应必须无事件与 payload。`OXVF` + 版本 + 32 字节零表示终止性插件故障，不带 PCM。
- Ready 的 `helperTimeConstraint` 表示调度申请结果。控制侧 `Session::diagnostics()` 和
  `Controller::diagnostics(epoch)` 提供非事务快照：合法完成块数、最大空闲等待/发送/
  响应等待/helper 处理耗时（ms）以及两条线程的调度申请结果。计时与原子统计只发生在
  IO/helper 线程，不在实时方法中增加统计。空闲等待不是每块队列等待时间；策略获准也不能
  保证内核永不降级或任意负载下满足 deadline。
- helper 在初始化后使用指定的 VST3 processingMode（缺省 Realtime），顺序处理，缺省起点为 0、playing=true。
  显式初始位置与逐块上下文见下文。每块稳定排序 noteOff → parameter → noteOn。处理器请求重新加载、
  修改 IO/latency/参数标识等时终止当前流；不在处理期间偷偷重配置。插件报告的 latency 和 tail
  在初始化时返回；原始端口不负责图 PDC、自动尾音截止或图路由；output_events 借用最后一次成功 receive 的 MIDI，
  下次 receive/故障清空。DSP reset 使用同一音频包的 reset 标志。
- close/drop 必须在控制线程执行：先原子停止，再 shutdown socket 中断在途 I/O，杀死 helper
  进程组并 wait/join。插件 destructor 即使卡住也不让 close 等到 block deadline。Port 的销毁
  同样必须延迟到控制线程，不能在 callback 中释放其预分配缓冲。故障恢复是显式新建会话，
  重用经校验的初始化 configuration；旧队列和旧 sequence 不跨新会话复用。

原始端口不保证结果在某个 callback deadline 前到达。queueDepth 是容量，不是固定音频延迟；
需要固定调度的 native 调用方使用下面的 ScheduledPort。原始端口不打开系统输出；
DAW 分配使用上文的 Vst3Plugin 工程适配器。

验证：`cargo test -p oxitone-vst3-host --features host,stream` 覆盖二进制边界、队列与零分配、
假 helper 的启动/处理挂起、退出、错序、NaN、缓慢响应、故障 latch 和 close/reap。
上面的 `--stream` 命令另用 Rust probe 验证真实 VestiGain、2208 个音频块、短块、排队和两次
独立进程生命周期。`OXITONE_VST3_STREAM_REPORT` 指定 benchmark JSON；它测无设备的 IPC
往返与端口调用时间，不是设备 callback、全工程负载、deadline 保证或其他厂商兼容认证。

## 固定延迟调度（SDK 底层）

### 精确播放上下文（stream 11）

`options.transport?: Vst3Transport` 指定新实例的初始工程位置；省略时从 0 开始，采用
options 的 tempo/timeSignature、playing=true。显式 transport 的 tempo/拍号同时用于
插件初始化和处理，优先于 options 中相应缺省值。`vst3-transport.schema.json` 和 Rust
`transport_wire::Transport` 规定 projectFrame、continuousFrame、projectBeat、barBeat、
tempo、timeSignature、playing 与可选 cycle；beats 均为四分音符单位，barBeat 不晚于
projectBeat。帧为非负 JS safe integer，预留 4096 帧溢出空间。非法数值、拍号、循环范围
或未知字段拒绝。cycle 只描述工程循环区域，不让 helper 自行执行跳转。

原生 `RealtimePort::submit_at(left,right,events,transport)` 把精确上下文与 PCM 原子入队。
后续普通 submit 按最后一个明确位置、累计整数帧和当前固定 tempo 推进；每块从同一
锚点重新计算 musical position，不逐块累加浮点 beat。playing=false 时冻结工程帧/beat，
continuousFrame 仍按真实处理帧数推进。调用方遇到 tempo 变化、定位或循环跳转时提供
新上下文；改变上下文不清空 processor voices/delay，不代替新 epoch/实例生命周期。
helper 在每次 process 前写入真实 VST3 ProcessContext，正确设置/清除 playing、cycle
和各字段的 validity flags。语义对应 Steinberg 的
[ProcessContext](https://steinbergmedia.github.io/vst3_doc/vstinterfaces/structSteinberg_1_1Vst_1_1ProcessContext.html)。

`ScheduledPort::process_at(input,transport,outLeft,outRight)` 与
`AudioSlot::process_at` 只接受对齐的完整插件块；已有部分输入或短块时锁存 InvalidBlock。
普通 process 仍支持任意短段，且不会重播上一块的显式上下文。三层实时方法保持零分配、
零释放、无锁、无 I/O；helper 的位置更新不在音频调用线程执行。

upstream vst3-host 0.9.0 缺少明确位置 setter，仓库在 `vendor/vst3-host` 保留 MIT 源码及
最小 ProcessPosition 扩展，来源/变更范围见该目录 OXITONE.md；不是系统 Cargo cache 修改。
原始依赖按 channel Vec 的长度处理 PCM，helper 现在在保留 capacity 的前提下调整全部
channel 长度，使 1/17/111 等短块实际只推进请求帧数。此修复同时用于离线 render。

macOS 对已关闭 peer 的 SO_RCVTIMEO 可能返回 EINVAL，即使最后一条响应仍在缓冲区。
IO 线程只有在零等待 poll 确认 POLLHUP 后才继续读取已缓存的响应；其余失败仍报错，
原总 deadline 始终保留。这避免把 helper 发回的具体初始化错误误报为进程崩溃。

`node scripts/smoke-vst3-transport.mjs` 从仓库源码构建无设备 VST3 conformance fixture，
实际 processor 在 PCM 中输出处理帧计数和上下文字段；交错短块、暂停、tempo 变化、回跳
与 cycle 清除均逐样本验证。`OXITONE_VST3_TRANSPORT_REPORT` 指定报告；这是 SDK 路径验证，
不能作为 Engine 图、DAW loop、厂商 editor 或真实设备 callback 验收。

`stream::schedule::ScheduledPort::prepare(port, schedule)` 在控制线程接管全新、未提交过的
RealtimePort，预分配输入拼块、输出槽和接收暂存。`schedule_wire::Schedule` 与 TS
`Vst3StreamSchedule` / `vst3-stream-schedule.schema.json` 一致：`scheduleVersion:1`、显式
`epoch`（0…Number.MAX_SAFE_INTEGER）和 `latencyBlocks`（2…16，无缺省值）。版本或未知
字段拒绝；queueDepth 必须至少等于 latencyBlocks。失败消耗并关闭传入端口，仍由控制线程
close/drop 对应 Session。此契约是调用侧调度配置，scheduleVersion 1 独立于 helper stream protocol 5。

- `process(Input{epoch,frame,left,right,events,payload},outLeft,outRight)` 每次接收 1…blockSize 帧。
  frame 是此 epoch 内从 0 连续递增的 u64 帧，事件偏移相对此次调用；并非插件的绝对工程
  播放位置。输入和输出两个声道长度必须相等。每次至多 256 事件、每个拼成的插件块也至多
  256 事件和 16384 bytes SysEx；跨短段累积超预算立即锁存 EventOverflow，不截断事件或部分接受当前调用。
  非有限 PCM、非法参数/MIDI、非零 instrument 输入、空块或长度错误都锁存 InvalidBlock。
- 短段拼成完整 blockSize，事件重定位到插件块内，保留同帧输入顺序；helper 再执行既有
  noteOff/parameter/noteOn 稳定排序。不向插件补虚构时间，也不在 segment 边界丢失音符。
  结束内容后调用方仍须显式送零输入推进积存音频和所需 tail；没有自动 drain/尾音结束判定。
- 调度延迟 L = latencyBlocks × blockSize，先输出 L 帧静音，之后第 n 个响应只在
  `[L+n×blockSize, L+(n+1)×blockSize)` 输出。每次调用入口最多取 queueDepth 个响应；
  本次调用涉及的到期响应必须在这次收集时可用，不等待、不在输出中途补拉结果。
  最小两块包含一块输入拼接开销，给 helper 留至少一块的标称处理窗口；并不保证 OS/插件
  一定满足它。`buffering_latency_frames()` 返回 L，`latency_frames()` 返回 L + 插件固有
  latency；插件自身延迟已在 PCM 中，不再额外插入第二份 delay，也没有裁头或全图 PDC。
- 到期缺块锁存 DeadlineMissed、立即静音本次完整输出（包括之前已拷贝的前半段）及后续
  调用，并原子停止旧流。迟到结果不补播，已缓存的后续结果也不使用；不会回放上一块、
  移动音频时间轴或自动 dry bypass。fault() 保留首个原因；Session 状态对应 DeadlineMissed、
  Invalidated 或 ScheduleFault，已有 helper 的 Crashed/TimedOut/PluginFault 等不会被覆盖。
- seek/loop/图替换先调用实时安全 `invalidate()`；它只锁存失效并原子停止，不销毁数据、
  关闭 socket 或等待 helper。epoch 不匹配、frame 跳跃/重复也终止失效。旧对象必须保留到
  控制线程回收，在控制线程 close Session 并销毁 ScheduledPort，再独立准备新 session/epoch。
  不存在原对象 reset/rearm；未消费音频、部分输入、事件及 processor 声音状态均不跨会话。
  epoch 由调用者分配，不能用它假装插件已 seek 到工程位置。新会话可通过 options.transport
  设置真实初始位置。工程后台执行器已连接 seek/tempo/meter/loop；原生 scheduled port 本身不重建状态。
- process/invalidate/accessors 不分配、释放、等待、I/O、读时钟或调用 JS。存储受最大
  16 块、4096 帧、256 事件边界约束；单次最多拆成两段。全部销毁仍必须在控制线程。
  输出缓冲非法时最多清零每声道 blockSize 个样本，防止错误长度造成无界实时工作。

验证覆盖 1/17/111/128 等不规则短段、2/4/16 块延迟、模拟插件额外 7 帧 latency 的逐样本
对齐、事件块边界、启动/迟到/段内失败整块静音、版本/容量/帧溢出、旧 epoch 隔离、20,000
次短段处理的零分配/释放，以及真实挂起 helper 在调度 deadline 后的 close/reap。
真实 fixture 运行方式：

```sh
cargo build --release -p oxitone-vst3-host --features host,stream \
  --bin oxitone-vst3-host --example vst3-schedule-probe
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3.mjs --schedule
```

`OXITONE_VST3_SCHEDULE_REPORT` 指定报告。probe 每 128 帧周期调用 17+111 两段、用不同
左右声道的确定性信号逐帧检查 256 帧偏移，关闭旧会话后重新准备第二个 epoch。计时范围只含
process，另记 caller interval 与 deadlineMisses；普通线程尽力按周期唤醒且不追赶错过的周期。
这些结果是无设备调度开销/PCM 验证，不代表系统 callback、全工程负载或真实 loop 验收。

## 实例交接与控制线程回收（SDK 底层）

`stream::managed::Controller::new(ManagerOptions)` 返回控制侧 Controller 与独占 AudioSlot。
ManagerOptions 对应 TS `Vst3StreamManager` / `vst3-stream-manager.schema.json`，要求显式
`managerVersion:1`、sampleRate（8000…192000）、blockSize（16…4096）和 capacity（2…16）。
这里只建立有界所有权队列，不加载插件；仍是 native SDK，不是可直接 Assign 的 Engine 节点。

- 控制侧 `prepare(helper,start,schedule,sessionOptions)` 同步完成 hash/能力校验、helper
  初始化和 ScheduledPort 预分配，再发布替换。所有实例共用 manager 的采样率与 blockSize，
  不能在会话内改变格式。成功发布的 epoch 必须严格递增，不回绕、不重复；失败不占用 epoch。
  `info(epoch)` 在激活前即可查询总延迟，`plugin_info(epoch)` 提供已验证的 Ready，便于后续
  graph adapter 在控制线程编译布局和 PDC。替换允许不同 latency，但这里不执行全图补偿。
- capacity 同时计算 pending、active 与 retired；helper 崩溃后仍占预算，直到对应端口回收。
  满容量在启动新进程前报 BudgetExceeded，不能挤掉当前实例或静默覆盖 pending；控制线程
  应定期 reclaim。prepare 会先回收已退休实例，加载失败不改变仍在播放的实例或替换队列。
- 音频侧在每个引擎 block 的首段前显式调用 `activate_next()`。它按 FIFO 取首个合格替换，
  返回 epoch 与总延迟；空队列返回 None，保留当前实例。若候选的故障在激活检查时已可见，
  返回 PreparedFault，将候选送回控制侧，保留原 active；检查后发生的崩溃仍由 process 故障
  静音处理，不能承诺崩溃与替换是原子操作。新实例从该 epoch 的 frame 0 开始，启动固定
  调度延迟的静音；不延续旧尾音，也无 crossfade 或无缝 seek。
- `require_epoch(epoch)` 是实时安全、单调递增的最低版本要求。它立即 invalidate 更旧 active，
  并通过 atomic floor 使尚在初始化的旧 prepare 完成后报 SourceChanged、关闭并回收 helper。
  activate_next 每次最多检查 capacity 个条目，将已排队的过期实例直接退休；不播放旧 PCM。
  低于已有 floor 或超出 JS safe integer 的请求报 InvalidEpoch，状态不变。相同 epoch 不是
  reset；重启必须申请新 epoch。普通 `invalidate()` 只停止当前实例，不取消 pending。
- 成功替换只移动 Box，并将旧 ScheduledPort invalidate 后放入 retired 队列。两个队列容量
  均为 capacity，控制侧在回收前不会解除 session 的预算预留；因此音频侧退休有保证的空位。
  不分配、不释放、不读时钟、不关闭进程、不加锁或等待。违反内部所有权不变量时终止 manager，
  遗忘异常对象以避免在实时线程析构；正常容量和所有权路径有零分配/释放测试。
- `AudioSlot::process` 遵循前述 ScheduledPort 帧与事件契约，不隐式激活实例。尚无 active 时
  输出静音、返回 NotPrepared；manager 已关闭时输出静音、返回 Closed；调度故障保留具体
  Fault。错误缓冲长度最多清除每声道 blockSize 帧。active()/fault() 仅供当前 Rust 音频线程
  读取，不是 JS 可共享的 mutable graph。
- 控制侧 `reclaim()` 取回 retired，close/join/reap 对应 Session，再释放 buffers 和预算。
  `shutdown()` 是终止性且幂等：先发布 closed，再关闭所有进程，清理 pending/retired。
  正在并发执行的音频调用可以完成其已观察到的状态；后续观察 closed 的调用静音且不等待
  控制清理。音频正在使用的 active buffers 仍由 AudioSlot 持有，不能随 Controller 提前释放。
  并发边界上迟到入队的退休条目由后续 shutdown 或返回控制线程的 AudioSlot 最终回收。
  Controller 和 AudioSlot 最终都须在控制线程销毁；先 drop AudioSlot 时，下一次 reclaim
  或 Controller drop 关闭剩余 queued helpers。关闭不依赖音频队列空位。

调用顺序是控制线程 new/prepare，Rust render 线程 activate_next/process；请求 seek/换图时
先 require_epoch，然后控制线程准备相同或更新 epoch，render 边界激活，控制线程 reclaim。
如果尚无合格替换，应按上层 transport 策略保持静音或暂停；不要把旧 epoch 的音频当作新位置。
绝对位置通过 process_at 单独传递；这些底层生命周期接口自身不负责 DAW pause、seek、tempo map
或无缝 loop，工程执行器负责对应映射。

验证包含队列容量/格式/epoch 拒绝、16 个候选的有界激活/过期清理、准备失败保留旧实例、
初始化期间 epoch 变化/AudioSlot 关闭、激活前 helper 崩溃、挂起 IO 下并发 shutdown、全部
helper 回收，以及交换/故障/过期清理的零分配与零释放。真实 fixture：

```sh
cargo build --release -p oxitone-vst3-host --features host,stream \
  --bin oxitone-vst3-host --example vst3-managed-probe
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3.mjs --managed
```

`OXITONE_VST3_MANAGED_REPORT` 指定结果；32 次真实进程初始化、31 次旧实例回收、各 epoch
40 个插件块，逐帧复核相反左右声道、固定延迟与旧 PCM 隔离。调度默认 2 块，可显式设置
`OXITONE_VST3_MANAGED_LATENCY_BLOCKS=8` 等 2…16 的值，queueDepth 为 max(4,latencyBlocks)，
两者都记录在报告中；不会在失败后自动增加缓冲。每次激活前/新 epoch 开始前
完成控制准备，音频调用按普通线程节拍推进；报告方法成本，不是并发工程编译负载或 HAL 验收。

## 能力范围与未完成验收

工程实例、DAW 分配、独立厂商配置编辑器、系统目录扫描、多 class 枚举和平台 helper 打包
已连接。扫描入口只列用户/系统目录；选择 bundle 后显式 Discover plugins 才加载 factory，
结果保留在会话的 `DocumentView.vst3Bundles`，上限 4096；scan/cancel 不修改源码 revision。
厂商原生窗口仍需解锁桌面后的 Apply/Cancel/缩放交互验收；Intel 包运行依赖对应 CI runner。
需继续扩大 reset 合规性、商业插件兼容性和负载覆盖。原生多总线、工程乐器辅助输出与
bus insert 多路输入、辅助输出和侧链已接入；动态 layout/latency/参数表
变化冻结当前实例后，可通过捕获状态的显式事务恢复、重新校验与 PDC 换图，见本文开头。
SDK 和 DAW 已能异步控制当前播放实例的厂商窗口；DAW 的 Use current state 已连接文档
捕获事务及 Undo/Redo/Save。Touch/Write helper、SDK take/source、DAW 参数录制入口、
Playlist 时间映射及源码事务已接入；录制曲线可继续做范围编辑。Channel 主 MIDI 输出已接入；主总线 SysEx 已接入，Note Expression、辅助事件总线、Mixer 反馈 MIDI 与 surround 仍未支持。
不能将单个插件、模拟设备或离线通过当作全量兼容性或听音证明。

### 效果器与音源的可用性回归

后续兼容性工作优先检查实际效果器和音源的加载、发声、参数、状态恢复与播放。对本机安装的
VestiGain / Vesti MIDI Synth，可运行以下联合回归；不打开音频设备：

```sh
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 \
OXITONE_VST3_INSTRUMENT=/path/VestiMIDISynth.vst3 \
node scripts/smoke-vst3-instrument.mjs
```

该脚本检验复音 note-on/off、音源到效果器的实际 PCM、两个实例的独立参数、播放前修改与
运行中 capture、显式接受状态和换图、保存重开及模拟 loop/seek/pause/resume/stop。
报告记录精确 class/hash、环境、PCM 误差与故障/欠载计数；不以无错误的静音输出冒充音源发声。
源码/DAW 分配另由 `smoke-vst3-daw.mjs` 验证 Undo/Redo/Save/reopen。
商业插件默认空资源状态仅能证明初始化、处理与捕获链路，需加载实际资源后再验收声音。
