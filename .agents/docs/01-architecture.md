# Oxitone 总体架构

发布前目标已改为 [代码/DAW 双向 authoring](../designs/source-daw/README.md)：Node Document
Service 拥有可变源码，GPUI 提交语义事务，Rust 继续独占实时执行。下文的 read-only、
core/native 耦合与旧协议描述是迁移基线，不再是目标限制。当前增量见
[source authoring 契约](15-source-authoring.md)，未完成的边界不能标记为协议 2。

## 分层原则

Node.js 最低支持版本为 24；workspace 与所有发布的 npm 包统一声明 `engines.node: >=24`，
CLI 工程打包以 Node 24 为目标，开发类型使用 `@types/node` 24，macOS arm64/x64 CI
均验证 Node 24。浏览器仍通过 `@oxitone/web` 运行；VS Code 扩展由其 Electron extension
host 执行，构建目标跟随该宿主，外部 CLI/Document Service 则要求 Node 24+。

代码模块化、文件大小和依赖方向遵循 [工程化规范](22-engineering.md)。手写文件聚焦单一职责，
目标 150–250 行，接近 300 行时按领域边界拆分；不通过压缩格式规避。

TypeScript 是声明式 authoring layer：负责创建对象、组合 patterns、注册插件、保存项目和发出控制命令。Rust 是 execution layer：负责把快照编译成不可变 render graph，执行调度、DSP、混音、设备输出和离线导出。

数据流只有一条主路径：

```text
TS builders -> validated ProjectSnapshot -> versioned N-API / Wasm memory command
             -> Rust validator/compiler -> immutable RenderGraph
             -> realtime block renderer -> CoreAudio / shared PCM + Web Audio / WAV writer
             <- TransportSnapshot / meters / diagnostics
```

TypeScript 对 authoring 对象拥有所有权；Rust 对编译后的图和实时状态拥有所有权。N-API 返回只读语义的序列化 snapshot，不返回可直接改变 Rust 状态的对象引用。

## 目标 workspace

```text
oxitone/
  packages/
    core/                 # @oxitone/core: TS domain models, builders, helpers
    protocol/             # @oxitone/protocol: versioned wire schemas and codecs
    midi/                 # @oxitone/midi: deterministic SMF Type 1 export
    samples/              # @oxitone/samples: sample metadata/editing facade
    native/               # @oxitone/native: low-level facade and platform binary resolver
    web/                  # @oxitone/web: Wasm memory ABI client + Worker/AudioWorklet host
    sdk/                  # oxitone: unified public authoring/native/sample entry
    native-generated/     # generated N-API TS declarations; never hand edit
    vst3/                 # @oxitone/vst3: isolated VST3 registration, discovery and offline SDK
    vst3-host-darwin-*/    # @oxitone/vst3-host-darwin-arm64/x64: optional native helper packages
    cli/                  # @oxitone/cli: render/export-midi/doctor and preview/watch runner
    editor-vscode/        # oxitone-vscode (private VSIX): VS Code buffers and Document Service adapter
  crates/
    core/                 # oxitone-core: IDs, units, errors, immutable data
    graph/                # oxitone-graph: validation and RenderGraph compiler
    transport/            # oxitone-transport: tempo, bars, event scheduling
    dsp/                  # oxitone-dsp: allocation-free DSP primitives
    instruments/          # oxitone-instruments: synth and instrument host
    samples/              # oxitone-samples: decode, edit, resample, cache
    mixer/                # oxitone-mixer: buses, sends, sidechain, effects
    render/               # oxitone-render: realtime and offline render engines
    io-macos/             # oxitone-io-macos: CoreAudio output adapter
    napi/                 # oxitone-napi: thin versioned bridge only
    wasm/                 # oxitone-wasm: import-free memory ABI, same Rust engine and static drum plugin
    vst3-host/            # oxitone-vst3-host: isolated macOS helper + optional bounded native stream
    bench/                # oxitone-bench: criterion + callback harness
    example-drums/        # oxitone-example-drums: unpublished C ABI drum-machine cdylib example
  apps/
    preview/              # oxitone-preview: GPUI read-only viewer; links engine crates directly
  schemas/                # JSON schema / protocol fixtures
  examples/               # small executable projects; offline is a private TS workspace package
  benches/                # scenario manifests and golden assets
  .agents/
```

The first npm package may bundle `@oxitone/core`, `@oxitone/protocol`, and the native resolver for ergonomics. Subpath packages remain separately testable and must not create circular dependencies.

统一入口 `oxitone`（packages/sdk）依赖 core、samples、native；可选 `oxitone/vst3`
子路径只转出 `@oxitone/vst3`，不会改变主入口运行时；core/samples/midi
依赖底层 `@oxitone/native`，不能反向依赖统一入口。保留原有低层函数导出并新增
Project、Pattern、音源 helpers、importSample 等 authoring 导出，无循环依赖。

仓库根是私有 `oxitone-workspace`，不能与公开 facade `oxitone` 同名；否则 pnpm
会混淆 workspace 依赖顺序，首次构建时可能在 native 类型生成前构建 core。

根 build/lint/typecheck 包含 `packages/*` 与 `examples/*`，按 workspace 依赖顺序
构建。`@oxitone/cli` 依赖 core 的项目文件加载器，避免重复实现便携工程版本/hash/
路径校验；读取 snapshot 与准备资源仍分别归 protocol 与 Rust。

## Package ownership

| Layer               | Owns                                                                                      | Must not own                                 |
| ------------------- | ----------------------------------------------------------------------------------------- | -------------------------------------------- |
| `@oxitone/core`     | IDs, TS builders, Chord/Arp, validation hints                                             | audio buffers, device handles, native state  |
| `@oxitone/protocol` | schema version, encode/decode, compatibility                                              | DSP decisions or mutable runtime state       |
| `@oxitone/vst3`     | optional local VST3 inspection/offline-render client and versioned helper contract        | realtime graph, DAW/GUI, editor ownership    |
| `@oxitone/samples`  | non-destructive edit descriptors and metadata                                             | decoding in JS or realtime transforms        |
| `oxitone-napi`      | command/snapshot bridge, error translation                                                | render loop, graph ownership, business logic |
| `oxitone-graph`     | graph validation、编译成不可变 `RenderPlan`（纯数据）、plugin manifests                   | OS APIs, N-API, file I/O, 插件实例所有权     |
| `oxitone-dsp`       | sample/block DSP primitives                                                               | allocation, locks, logging, clocks           |
| `oxitone-io-macos`  | CoreAudio setup/callback/device enumeration                                               | project semantics or plugin policy           |
| `oxitone-render`    | 插件实例与运行时状态、transport-driven graph rendering、offline WAV sink、SMF MIDI export | TypeScript objects, UI concerns              |
| `oxitone-transport` | tempo map/time signature、事件调度、automation evaluator 与 tempo lane 烘焙               | 音频 buffer、插件实例                        |
| `oxitone-preview`   | GPUI viewer 状态归约、scope 分析（UI 线程）、transport 交互                               | 音频语义、authoring 写回、编辑能力           |

## Plugin model

Instruments and effects implement the same conceptual lifecycle in Rust and advertise a matching TS manifest:

1. `descriptor()` is static metadata: stable plugin ID, version, channel layout, parameter specs, tail policy.
2. `prepare(PrepareContext)` runs off the audio thread when sample rate/block size changes.
3. `process(ProcessContext)` runs per block and is realtime-safe.
4. `reset()` is a realtime-safe flush used by seek/loop; `dispose()` runs on the control thread.

Phase 1 plugins implement the stable C ABI in `08-plugin-abi.md` and ship in two forms: statically linked Rust crates registered at build time, or dynamically loaded `.dylib` distributed through npm platform packages. Built-in instruments/effects use Rust traits with the same descriptor/lifecycle contract; third-party libraries use the C entry table and an owned adapter. The optional `@oxitone/vst3` package starts a separate Rust helper for exact-class inspection, project instruments/inserts and offline WAV rendering, and reads/writes explicit preset files. Project registration and DAW source transactions carry hash-pinned class metadata and configuration. `oxitone-render` uses an explicit isolated background executor for these nodes; the hardware callback only consumes completed PCM. Ordinary built-in/C graphs retain their hard realtime executor. The vendor editor and current playback-instance controls return captured state through source transactions. Stream 11 preserves physical mono/stereo bus slots and activation, with Project bus-insert sidechain support and Touch/Write recording. Engine 1.4 routes instrument auxiliary outputs through Channel.outputRoutes with shared events, independent PDC and explicit activation; Engine 1.6 routes Mixer insert auxiliary inputs and outputs by stable instance identity, with activation, complete PDC and stems. Engine 1.5 adds explicit automation priority for DAW recording layers, whose editable Sources and Playlist clips use the shared source transaction and Undo/Save history. Engine 1.7 routes Channel instance MIDI output to downstream native instruments in the same segment; stream 11 carries bounded MIDI and SysEx payloads in both directions. MIDI graphs always use background execution and preserve channel-ID audio summation order. None of these extensions change C ABI v1. See `24-vst3-sdk.md`.

The native `stream` feature adds a persistent isolated helper and preallocated Rust PCM/event queues.
Only a background IO worker accesses sockets/processes; the realtime port only validates, copies and
queues bounded blocks. The native ScheduledPort assembles segments, schedules results at an explicit
fixed sample latency, and latches silence on missed deadlines or epoch discontinuity. It reports total
latency but has no graph PDC or absolute seek/loop binding. Invalidation requires control-side retirement
and a fresh session. Controller/AudioSlot provides bounded prepared replacements, a monotonic epoch
floor and control-side process/buffer retirement. It does not bind the slot into RenderGraph or implement
transport/PDC. Queue capacity does not select latency. The client builds without the VST3 loader.

第三方 npm 分发（对应产品目标"用户按标准自行开发音源/效果器并经 npm 分发"）在 Phase 1 的落地方式：

- 插件作者发布 TS manifest 包（`ParameterSpec[]`、元数据、`pluginPath()`）加平台 dylib 包（如 `@acme/oxitone-osc-darwin-arm64`）。
- 用户 `npm install` 后调用 `registerPlugin(engine, { libraryPath: plugin.pluginPath(), manifest: plugin.manifest })`；加载、ABI 校验、descriptor 比对和 prepare 都在控制线程完成，callback 永不触发加载。
- 动态插件与内置插件遵守同一 realtime contract；engine 无法强制第三方代码，提供返回码/非有限值/latency 变化的 mute-node 与插件 fault 计数；逐节点 deadline watchdog 尚待实现，详见 `08-plugin-abi.md`。
- 对稳定性要求最高的应用可以只用静态链接插件；动态加载由 engine option 控制。

任何插件不得在 descriptor 之外接收未声明参数；跨版本参数迁移由插件自身在 `prepare` 前声明。

## State and concurrency

- Authoring state: mutable in TS, serialized with monotonically increasing `revision`.
- Compile state: immutable `ProjectSnapshot` validated off-thread; compiler produces `RenderGraph`.
- Realtime state: preallocated graph state, transport cursor, voices, delay lines and meters.
- Control queue: bounded SPSC/MPSC lock-free queue from control thread to audio thread. Commands carry revision and sample-frame target.
- Meter queue: bounded lock-free queue from audio to control thread; dropped meters are acceptable, dropped transport acknowledgements are not.
- Graph swap: prepare a complete graph off-thread, atomically publish it, retire the old graph after the callback leaves it.

## IDs, units, and compatibility

All entities use opaque, globally unique string IDs (`trk_`, `pat_`, `chn_`, `mix_`, `smp_`, `auto_`). IDs are stable across serialization and never array indexes. Authoring uses beats/bar positions as rational numbers; DSP uses `u64` sample frames; wall-clock seconds appear only in device/render settings. Wire messages include `protocolVersion` (`major.minor`) and reject unknown major versions.

## Architectural non-goals

- No MIDI import in Phase 1.
- No microphone/input capture.
- No second DSP implementation in JavaScript. Wasm/Web Audio reuse the Rust engine
  through the host boundary specified in `12-wasm-web-audio.md`.
- No shared mutable TS/Rust object graph.
- No GUI editor required for the SDK release.
