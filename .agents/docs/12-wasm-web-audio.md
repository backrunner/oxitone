# Wasm 与 Web Audio 宿主

用户要求扩展 macOS-first 架构到 Wasm runtime / Web Audio。复用 Rust core、graph、
transport、DSP、samples、instruments、mixer、render；不创建第二套 JavaScript 音频实现。

## 分层与能力

- `oxitone-wasm`（`crates/wasm`）编译为 `wasm32-unknown-unknown`，导出独立版本的
  C 风格 memory ABI；不依赖 JavaScript imports、WASI、文件系统、系统线程或设备。
- `@oxitone/web`（`packages/web`）提供 typed Wasm 控制、内存资产、离线输出与
  Web Audio session。Node 可以实例化同一个模块做测试/离线导出。
- Web Audio 使用 Worker 调用 Rust process，AudioWorklet 仅消费共享 PCM ring；
  authoring、JSON、编译、解码、DSP 不进入 AudioWorklet 的 process。Web Audio
  强制要求的 JavaScript 宿主适配不承担音乐语义。需要安全上下文与跨源隔离。
- 浏览器复用 Project/Pattern/轨道/Mixer/Automation authoring 和 snapshot 协议。
  Node 文件、N-API、CoreAudio 与 GPUI 是平台宿主，不编译进 Wasm；对应能力使用
  内存资源、下载的 WAV/MIDI 和 Web Audio 输出。任意 macOS dylib 不能直接加载。
  自带鼓机静态编译相同 C ABI 实现；第三方需重编译并集成，不能冒充已支持动态 Wasm 插件。

## ABI v1 与生命周期

每个 Wasm instance 独占一个引擎，不跨实例共享 Rust 指针。control 命令携带
`protocolVersion: "1.0"`，使用受限 JSON 输入与结构化错误。这是独立 Wasm control 外壳；
内部 ProjectSnapshot 已支持 engine 1.1 的插件实例/scoped automation（见 [20](20-plugin-instances.md)），
不把 Wasm memory ABI 1 或 control 1.0 重新标成 1.1。
宿主通过 alloc/free 管理输入区域，读取 response pointer/length 后复制结果；
不得保留到下次控制命令后。PCM 指针在 compile 后查询，直到下次编译/释放保持有效。
指针数值有效不代表 JS TypedArray view 有效：任何 control 都可能 grow memory，TS facade
因此在 control 后重建 PCM views；宿主应在 process 前重新获得所需 view。
宿主必须传入自身有效分配的指针和准确长度，不得在同一实例并发或重入调用。

compile 先准备完整新图，失败保留上次合法图。资产通过 bytes 导入、Rust 解码/SHA-256
校验、按 snapshot edits 与采样率准备；不自动读取 assetUri 或发起网络请求。
process 只消费预分配状态，禁止内存增长/分配/释放、锁、JSON、时钟和宿主调用。
transport/parameter 命令在 process 外校验；WAV/MIDI 导出在控制侧生成 bytes。

ABI 导出（Wasm32 指针/长度均为 u32，所有调用均同步）：

| Export | 合约 |
| --- | --- |
| `oxi_abi_version() -> u32` | 当前 1 |
| `oxi_alloc(len) -> ptr` / `oxi_free(ptr,len)` | 输入必须来自本实例活跃分配；一次分配只释放一次；len=0 或 >64 MiB 返回空指针 |
| `oxi_command(jsonPtr,jsonLen,dataPtr,dataLen) -> u32` | 0=成功，1=结构化错误；JSON ≤16 MiB，binary ≤64 MiB；binary 只供 importSample |
| `oxi_response_ptr/len()` | UTF-8 `{protocolVersion:"1.0",ok,value}` 或 `{protocolVersion:"1.0",ok:false,error:{code,message,path}}` |
| `oxi_binary_ptr/len()` | 最近一次成功 WAV/MIDI 输出；下一 control 可使其失效 |
| `oxi_left_ptr()` / `oxi_right_ptr()` | 两个预分配 planar f32 block，各 blockSize frames |
| `oxi_process() -> u32` | 0=正常，1=未编译，2=graph fault；不做 control 工作 |
| `oxi_allocations()` / `oxi_deallocations()` | Wasm 实例 allocator 累计 u32 counter（允许 wrap），供 process 检查 |

命令均有 `protocolVersion:"1.0"` 和 `type`：`compile{snapshot}`、
`importSample{format}`（外加 binary）、`state`、
`transport{command:play|pause|stop|seek,frame?:decimalString,loopRegion?:[start,end]}`、
`setParameter{entityId,parameterId,value,atFrame?:decimalString}`、
`renderWav{frames:u32,startFrame?:decimalString,bitDepth?:16|24|32,dither?:boolean}`、
`exportMidi{options?:{ppq,tempoEventResolutionTicks}}`、
`resolveBeatDuration{snapshot,startBeat:BeatWire,durationSeconds}`、`dispose`。
位置范围为 0..2^53-1，loop 必须 end>start，默认 WAV 为 float32，无 float dither。
WAV convenience 导出输出 PCM ≤256 MiB，不改变 active graph/transport；构建产物设定
1 GiB memory 上限。普通 JSON/资源/graph 验证失败可恢复；Wasm trap/OOM 需要新实例，
不能保证保留该实例的旧图。dispose 释放图和资产，原始 ABI 允许后续重新编译；TS dispose
终止 wrapper 生命周期，后续调用报 InvalidProject。

## 输出与验收

Web Audio 采样率以 AudioContext 实际值为准；项目不匹配时显式拒绝，不能错误播放。
共享 ring 有界，producer/consumer 各自拥有游标，以 Atomics 发布；短缺输出零并累计
underrun。pause/seek/换图必须 flush 旧 PCM，不能在新位置混入缓存音频。
无效代码或快照不替换已接受工程。关闭 session 终止 Worker、断开节点并释放自有 context。

ringFrames 为 512..65536 的 2 次幂，默认 4096；render horizon 至多 2048，blockSize
必须放入 ring/horizon。flush 递增 epoch，consumer 推进自己的 READ 到 flush target 并
发布 ACK 后 producer 才能写新 PCM；期间输出零。展示游标追踪实际消费帧，不能从已
wrap 的 render-head 简单减去 bufferedFrames（会跳过首次 loop 前的段落）。
Worker 的普通 compile 先构建后换图，但大型编译会占用同一 producer：可能 drain ring；
当前没有双 Worker 无缝准备保证。源代码 watch 的 build/runtime 异常保留工程，且只在
Rust 接受后更新展示；无限循环 authoring 需调用方的有界 Worker，不承诺自动抢占。

验收包括真实 Wasm 实例有声处理、native/wasm PCM 容差对拍、内存样本/效果链/MIDI/WAV、
参数事件/seek/失败恢复、process 内存稳定、实际浏览器 AudioWorklet 输出与有声 demo。
记录 Wasm/Worker microbench、浏览器 underrun 和运行环境；不把这些记录当成设备长测。

测试约束：用户要求测试不走系统音频设备。浏览器自动化必须在任何图连接前使用
AudioContext sinkId:{type:'none'}，不可用即失败；启动 Chromium 另加 --mute-audio。
报告记录 sink.type=none，检查真实 PCM 和 worklet 上游数据。原生集成测试明确选择
audioBackend=simulated，保留 worker/ring/transport/latency 断言而不打开 CoreAudio。
