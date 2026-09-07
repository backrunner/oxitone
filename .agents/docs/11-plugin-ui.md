# Plugin UI：通用详情、声明式界面与原生扩展

状态：通用详情窗口已实现；下文 P1/P2/P3 为后续规划，尚未提供对应 SDK、manifest schema 或 C 入口。
本方案补充 `08-plugin-abi.md` 与 `09-preview-app.md`，不改变现有 audio ABI v1。

## 已交付的基础层（P0）

- 每个音源和 Channel/Mixer/Master insert 都可打开只读 GPUI 详情窗口；相同地址聚焦已有窗口，多个地址可同时打开。
- 控制线程从已校验 registry 复制 descriptor 与动态库路径/实际 SHA-256，详情不创建 DSP 实例，不持有库或音频实例指针。
- 显示源码初始参数、缺省参数、范围/单位/平滑/rate/mapping、自动化绑定、mix/bypass、音源资源和 structured state；用户可以筛选、滚动、切换标签及复制引用 JSON。
- 有效 watch 快照更新所有窗口；拒绝的构建不改变窗口数据。窗口键是 owner kind + owner ID + instrument/insert index。EffectRef 无独立 ID，重排后窗口跟随槽位，不能用 pluginId 作为实例 ID；同插件的多个实例必须互不混淆。
- 关闭详情仅销毁该窗口；关闭主窗口退出 session。详情跟随系统 appearance，使用自绘标题区和原生交通灯。

## 选择：先提供由 GPUI 渲染的声明式 UI，再提供可选原生 UI

| 形式 | 提供方 | 优点 | 代价 / 适用范围 |
| --- | --- | --- | --- |
| 通用详情（当前） | 宿主按 descriptor 生成 | 所有内置与 dylib 插件立即可查看 | 不表达插件自己的视觉结构 |
| 声明式 UI（P1，优先） | npm 插件包附带版本化 JSON 和本地资源 | 自定义布局/品牌、统一主题和只读约束；不执行第三方 UI 代码 | 受宿主组件集合约束 |
| 原生 UI（P2，可选） | 插件包附带 UI companion dylib | 插件可提供自己的 AppKit/Metal 视图 | 平台专用；同进程原生代码故障可能拖垮整个 viewer |

GPUI 是宿主窗口和声明式组件的渲染层，不把 GPUI/Rust trait 或 Entity 通过 dylib ABI 暴露。
不要求插件作者与宿主使用同一 Rust 编译器或 GPUI git revision，也不要求提供浏览器/JavaScript UI。

## P1：独立的 UI 注册与布局协议

未来在 `RegisterPluginOptions` 增加可选 UI 注册信息，保持 DSP manifest 与音乐快照不变。
现有 `pluginManifestSchema` 不支持自定义 UI；不能仅在当前 manifest 写入下例就认为会生效。
实现必须先同步 `packages/protocol/src`、Rust wire、runner IPC、生成 schema 与兼容性 fixture。

建议的注册形状（草案，不是可调用 API）：

```ts
ui: {
  protocolVersion: '1.0',
  kind: 'declarative',
  manifestPath: '/explicit/local/path/ui.json',
  expectedHash: '<sha256>'
}
```

独立 UI manifest 绑定精确 pluginId/pluginVersion 与有序参数表的规范化 hash，声明：

- 逻辑窗口尺寸与 min/max 尺寸，布局 root；初始组件包括 tabs、group、grid、label、parameter readout/knob/fader、meter、envelope/curve plot、sample overview。
- 参数绑定使用 descriptor 的稳定 parameter ID；单位、范围、默认值仍由 descriptor 提供，UI 不能覆盖语义。mix/bypass 等宿主参数属于独立 `host` namespace。
- resource ID 对应包内相对路径、SHA-256、类型、大小和自然尺寸。只允许显式本地文件，禁止远程 URL、目录扫描、路径逃逸；解析/解码在非实时线程完成。
- 语义颜色 token（surface/text/accent/warning 等）、light/dark variant 和文本可读性约束；不以硬编码字体假定所有机器均有该字体。
- 组件树深度、节点数、图片尺寸/解码字节、曲线点数均有预检预算；无法支持的必要组件回退通用详情，不能阻断 DSP 注册/编译。
- UI 状态（选中 tab、滚动、缩放）仅在 viewer 内保存，不改变 ProjectSnapshot。Preview 传入不可变 `readOnly=true`，所有写参数控件仅提供读数与显示交互。

参数数据分清 `source`、`default`、未来 `effective`。P1 先消费 P0 的 source/default；绑定到未提供的实时数据必须显示“不可用”，不得将初始值标作 live 或用零值伪装数据。

UI 包更新只重建 UI，DSP 二进制修改仍遵循显式 rebuild/register 规则。旧宿主不识别可选 UI 时使用通用详情，不自动下载兼容组件。

## P2：独立版本的原生 UI C ABI（macOS-first）

建议入口 `oxitone_plugin_ui_entry_v1`，UI ABI 与 audio ABI 独立协商；该 symbol 目前不加载、不调用。
优先独立 `lib<plugin>_ui.dylib`，DSP dylib 的 audio entry 不变。宿主按显式注册路径/hash/签名策略验证 UI library，平台包与 DSP 包一起分发，不能静默放宽 `signed-only`。

每个入口/host 表都以 `abi_major, abi_minor, struct_size` 开头，仅传固定宽度整数、计数字节切片、C 函数指针及 opaque UI handle。初始接口职责：

| 操作 | 所有权与线程 |
| --- | --- |
| query capabilities / size | 主 UI 线程；协商 `macos-nsview`、只读模式、主题和尺寸支持；无 DSP 实例 |
| create | 主 UI 线程；独立 UI 实例接收复制的插件身份、slot key、graph generation 和只读上下文 |
| attach | 宿主创建窗口及原生容器 NSView，插件附加自己的 NSView；parent 为借用对象，不得销毁宿主窗口 |
| update snapshot | 主 UI 线程；版本化、带 revision/generation 的有界 source/default 数据；调用期借用，插件要持久保存则复制 |
| resize / scale / appearance / visibility | 宿主传逻辑 points、backing scale、语义主题及可见性；插件不能同步进入嵌套窗口事件循环 |
| detach / destroy | 主 UI 线程；先移除子视图、取消回调/异步任务，再销毁 UI handle；最后释放该 UI library 引用 |

GPUI 继续拥有窗口、自绘标题区和交通灯。原生内容在独立的 AppKit 容器中承载，不把外部 NSView 误当 GPUI element；输入焦点、IME、Tab、Escape/⌘W、缩放和窗口关闭须有原型验收。

UI host 表不提供音频实例指针、render graph 指针、process/reset 调用或任意参数 setter。
通用能力只有按已声明 ID 订阅快照、请求合法显示尺寸和日志/诊断（非实时线程）。UI 写请求在 Preview 一律拒绝；未来若有可编辑产品，必须另行定义代码/authoring 的回写契约，不能绕过“代码为事实来源”。

graph swap 后 slot key 仍是显示地址，generation 必须更新；旧 generation 的回调/数据丢弃。
UI library 活期与 DSP library 活期相互独立，禁止因窗口关闭释放仍在播放的 DSP；也禁止为显示而创建第二个 DSP 实例。UI 关闭不停止 transport，主窗口退出必须回收全部 UI。

版本不兼容、缺失 symbol、初始化失败或 UI 资源错误：显示局部诊断并回退通用详情，保持合法音频图。
同进程 native UI 的野指针、阻塞和崩溃无法靠 C ABI 验证隔离；P2 不能宣称具备崩溃隔离或超时抢占。若发布要求第三方 UI 故障不影响音频，应另设 UI helper 进程与 IPC/surface 协议门禁。

## P3：可选实时参数/专属可视化反馈

- 先定义版本化 `PluginUiSnapshot`，每包带 engine/session、slot、graph generation、audible frame、序号和 drops。只有同代数据能进入当前窗口。
- effective 参数必须来自 Rust 引擎实际合并 host/automation、mapping 后的值；若要承诺平滑后数值，插件必须明确提供对应遥测，不能在 UI 用 source AST 猜测。
- prepare 期建立固定容量 ring/atomic 和订阅表；worker/audio 只写有界数值，无 allocation、锁、日志、JSON、N-API 或 UI 函数调用。控制/UI 线程节流读取并组装快照；满队列丢新可视化帧并计数。
- meter、频谱、包络、波表/鼓机状态使用可协商数据类型与预算。FFT/绘制在 UI/分析线程；窗口最小化或关闭后停止非必要采集。
- 图替换、参数表变化与插件故障使对应反馈失效，不能显示旧值为当前值。

## 分阶段出口

1. **P1**：TS/Rust/schema round-trip；未知 ID/预算/坏资源/旧宿主 fallback；内置 Wavetable 与真实 example.drums、fixture.gain 的声明式面板；双主题、多窗口、watch、只读约束和布局基准。
2. **P2**：公开 UI C header + C/Rust companion fixtures；动态库真实加载、ABI prefix/minor、主题/DPI/resize/焦点/IME、关闭再开、换图/移除、先销毁视图后卸载库；失败不换音频图；未锁屏 AppKit 实机验收。
3. **P3**：frame/generation/乱序/drops 测试，source/default/effective 区分；关闭窗口停止订阅；音频 PCM parity、allocation/free=0、开启多个 UI 的 worker 和真实 callback p95/p99/xrun 长测。

当前仅 P0 交付。P1/P2/P3、第三方 UI 隔离及正式签名分发没有由本次详情窗口工作自动完成。
