# Oxitone 总体架构

## 分层原则

TypeScript 是声明式 authoring layer：负责创建对象、组合 patterns、注册插件、保存项目和发出控制命令。Rust 是 execution layer：负责把快照编译成不可变 render graph，执行调度、DSP、混音、设备输出和离线导出。

数据流只有一条主路径：

```text
TS builders -> validated ProjectSnapshot -> versioned N-API command
             -> Rust validator/compiler -> immutable RenderGraph
             -> realtime block renderer -> CoreAudio / WAV writer
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
    native/               # oxitone: public facade and platform binary resolver
    native-generated/     # generated N-API TS declarations; never hand edit
    cli/                  # @oxitone/cli: render/export-midi/doctor
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
    bench/                # oxitone-bench: criterion + callback harness
  apps/
    preview/              # oxitone-preview: GPUI read-only viewer; links engine crates directly
  schemas/                # JSON schema / protocol fixtures
  examples/               # small executable projects
  benches/                # scenario manifests and golden assets
  .agents/
```

The first npm package may bundle `@oxitone/core`, `@oxitone/protocol`, and the native resolver for ergonomics. Subpath packages remain separately testable and must not create circular dependencies.

仓库根是私有 `oxitone-workspace`，不能与公开 facade `oxitone` 同名；否则 pnpm
会混淆 workspace 依赖顺序，首次构建时可能在 native 类型生成前构建 core。

## Package ownership

| Layer | Owns | Must not own |
| --- | --- | --- |
| `@oxitone/core` | IDs, TS builders, Chord/Arp, validation hints | audio buffers, device handles, native state |
| `@oxitone/protocol` | schema version, encode/decode, compatibility | DSP decisions or mutable runtime state |
| `@oxitone/samples` | non-destructive edit descriptors and metadata | decoding in JS or realtime transforms |
| `oxitone-napi` | command/snapshot bridge, error translation | render loop, graph ownership, business logic |
| `oxitone-graph` | graph validation、编译成不可变 `RenderPlan`（纯数据）、plugin manifests | OS APIs, N-API, file I/O, 插件实例所有权 |
| `oxitone-dsp` | sample/block DSP primitives | allocation, locks, logging, clocks |
| `oxitone-io-macos` | CoreAudio setup/callback/device enumeration | project semantics or plugin policy |
| `oxitone-render` | 插件实例与运行时状态、transport-driven graph rendering、offline WAV sink、SMF MIDI export | TypeScript objects, UI concerns |
| `oxitone-transport` | tempo map/time signature、事件调度、automation evaluator 与 tempo lane 烘焙 | 音频 buffer、插件实例 |
| `oxitone-preview` | GPUI viewer 状态归约、scope 分析（UI 线程）、transport 交互 | 音频语义、authoring 写回、编辑能力 |

## Plugin model

Instruments and effects implement the same conceptual lifecycle in Rust and advertise a matching TS manifest:

1. `descriptor()` is static metadata: stable plugin ID, version, channel layout, parameter specs, tail policy.
2. `prepare(PrepareContext)` runs off the audio thread when sample rate/block size changes.
3. `process(ProcessContext)` runs per block and is realtime-safe.
4. `reset()` is a realtime-safe flush used by seek/loop; `dispose()` runs on the control thread.

Phase 1 plugins implement the stable C ABI in `08-plugin-abi.md` and ship in two forms: statically linked Rust crates registered at build time, or dynamically loaded `.dylib` distributed through npm platform packages. Built-in instruments/effects use Rust traits with the same descriptor/lifecycle contract; third-party libraries use the C entry table and an owned adapter. The TS manifest is the compatibility seam for future WASM/VST3 adapters; the ABI and manifest must not expose Rust-specific types.

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
- No browser audio implementation as a second runtime.
- No shared mutable TS/Rust object graph.
- No GUI editor required for the SDK release.
