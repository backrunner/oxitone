# Oxitone Plugin ABI 与动态加载

本文定义第三方 Instrument/Effect 的分发与加载契约。目标是：用户 `npm install` 一个插件包后显式注册即可使用，不需要重新编译 oxitone 本体。VST3/WASM 是 Phase 2 的 adapter，复用本文的 manifest 与参数模型，不改变本文 ABI。

## 加载模型

- 动态插件是遵循 Oxitone Plugin ABI 的 `.dylib`，只在控制线程通过 `dlopen` 加载；audio callback 永远不触发加载、解析符号或初始化。
- 注册方式是显式的：`engine.registerPlugin({ libraryPath, expectedHash? })`。不做全局目录扫描，路径由 npm 包解析提供；`expectedHash` 不匹配时拒绝加载。
- ABI major 不匹配返回 `PluginAbiMismatch`；minor 向前兼容，插件声明 `minHostVersion`。
- 加载后 Rust 侧校验 `descriptor()`，并与 TS manifest 比对：pluginId、version、参数 ID 集合、channel layout 不一致返回 `PluginManifestMismatch`。
- 同一 `pluginId + pluginVersion` 重复注册是幂等的，返回已注册句柄。
- 内置音源/效果器就是静态链接的 ABI 插件，与第三方动态插件共享同一份 contract 和测试；不允许内置插件走私有路径。

## npm 分发格式

```text
@acme/oxitone-osc/                  # TS manifest 包（平台无关）
  package.json                      # exports pluginPath(); oxitonePlugin 字段声明 pluginId/version/abiVersion
  index.js / index.d.ts             # ParameterSpec[]、默认参数、pluginPath() 解析平台包
@acme/oxitone-osc-darwin-arm64/     # 平台包
  lib/libacme_osc.dylib
  manifest.json                     # pluginId, pluginVersion, abiVersion, sha256, minHostVersion
```

- 平台包命名、postinstall 解析和 ABI 检查与 `06-format-and-export.md` 的 native 包规则一致；禁止运行时从网络下载二进制。
- macOS release 分发的 dylib 必须签名并随宿主分发流程公证；dev 构建可以 unsigned（Node 官方二进制带 `disable-library-validation`，可加载未签名动态库），但 release 门禁不允许。
- 经浏览器等渠道手动下载的文件可能带 quarantine 属性，会被 Gatekeeper 拦截；npm 安装路径不产生该属性，文档需说明这一差异，不提供绕过 Gatekeeper 的脚本。

## ABI v1

ABI 是 C ABI，不是 Rust ABI：所有跨边界类型 `#[repr(C)]`，不暴露 Rust trait object、String、Vec 或泛型。插件可用任意语言实现，只要导出约定符号。

```c
// 插件导出唯一入口符号；host 以 abiVersion 决定如何解释返回结构。
const OxiPluginEntryV1* oxitone_plugin_entry_v1(void);

typedef struct {
  uint32_t abiMajor;             // = 1
  uint32_t abiMinor;
  const char* pluginId;          // 静态存储期，host 不释放
  const char* pluginVersion;
  OxiPluginKind kind;            // instrument | effect
  OxiChannelLayout inputLayout;  // effect 的输入；instrument 为 none
  OxiChannelLayout outputLayout;
  uint32_t capabilities;         // sidechainInput | reportsTail | ...
  const OxiParamSpecV1* params;  // stable parameter ID、unit、range、default、smoothing、rate
  uint32_t paramCount;
  OxiInstance* (*create)(const OxiHostContextV1*);
  void (*dispose)(OxiInstance*);
  void (*prepare)(OxiInstance*, double sampleRate, uint32_t maxBlockSize); // 控制线程
  void (*process)(OxiInstance*, const OxiProcessContextV1*);               // audio 线程
  void (*reset)(OxiInstance*);                                             // 控制线程
  uint64_t (*tailFrames)(const OxiInstance*);
  uint64_t (*latencyFrames)(const OxiInstance*); // 处理延迟，参与 host 的 PDC
} OxiPluginEntryV1;
```

- `OxiProcessContextV1` 携带 non-interleaved `f32` 输入/输出 buffer 指针、frame 数、排好序的 note/parameter 事件列表和 sidechain 输入指针；所有 buffer 由 host 预分配并拥有，插件不得保留指针、不得越界写、不得在 `process` 中分配或加锁。
- 参数变化以 sample-accurate 事件传入，插件按 `ParameterSpec.smoothing` 自行平滑；host 不在 ABI 层做隐式平滑。
- `tailFrames` 供 offline render 和 graph swap 决定尾音长度；不声明 tail 的插件在 reset/切换后立即静音。`latencyFrames` 上报处理延迟（lookahead、oversampling 等），host 用它做全图 PDC；延迟变化只能在 prepare 时改变，process 中途改变视为 fault。
- 第三方插件的 TypeScript 侧只提供 manifest 和类型，不能要求用户在 TS 里编写实时回调。

## Realtime 约束与故障归因

动态插件的 `process` 与内置插件遵守同一 realtime 不变量（`03-audio-runtime-spec.md`），但 host 无法强制第三方代码合规，因此必须提供兜底：

- callback watchdog 按 node/pluginId 归因 deadline miss 和 fault；策略默认 `mute-node`：该插件节点输出静音、图上其余部分继续运行，并向控制线程上抛结构化诊断。
- debug/diagnostic build 对插件输出做 NaN/Inf 检测，命中时按 fault 处理并记录 pluginId。
- 插件崩溃不能摧毁 host 进程是目标而非保证：Phase 1 插件与 host 同进程，`engine` 提供 `allowPlugins: 'signed-only' | 'any'`（默认 `signed-only` 仅在发布构建强制）。进程外沙箱和 WASM 隔离属于 Phase 2。

## 测试与 conformance

- 仓库提供 ABI fixture 插件（一个 Rust cdylib、一个纯 C 参考实现）和 conformance 测试套件：descriptor 校验、参数事件排序、prepare/reset 复用、tail 上报、manifest 比对。
- 同一插件分别以静态注册和动态加载运行，golden WAV 必须一致。
- 第三方插件发布前应在 CI 跑该套件；oxitone 版本升级时以 ABI major 决定是否破坏兼容。
