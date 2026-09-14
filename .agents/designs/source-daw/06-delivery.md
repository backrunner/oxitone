# 实施分层、迁移与验收

## 1. 代码与发布包归属

先沿用已有 package/crate 名称，避免为了设计引入另一个 workspace 包映射。
新增模块按职责拆文件，每个文件约 300 行以内；不把所有编辑逻辑塞进 preview runner。

| 所在层                                    | 目标职责与改动                                                                        |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| `packages/core` / `@oxitone/core`         | 纯声明式 Project/Source/Config/Instance/Target，生成与编辑语义、序列化；浏览器可用    |
| `packages/protocol`                       | document format 1、engine/IPC protocol 2.0、插件/UI 描述 schema 与生成器              |
| `packages/cli`                            | Document Service、TS Program/来源插桩/writer、watch/事务/恢复、插件 resolver/安装任务 |
| `packages/native`                         | 新 typed control facade 与平台加载；保留薄桥，不实现 source writer                    |
| `packages/samples`                        | 显式资源导入 facade；音频解码继续调用 Rust                                            |
| `packages/midi`                           | 复用统一 source/事件展开后的导出入口，删除重复音乐语义                                |
| `packages/sdk` / `oxitone`                | 统一导出 authoring 与 runtime facade，CLI/GUI 外也能执行保存的 TS                     |
| `packages/web`                            | 使用同一 document/随机/参数模型，经 Wasm 控制接口运行                                 |
| `crates/core/graph/transport`             | typed wire/targets、来源窗口、确定性事件/随机、automation DAG 与编译验证              |
| `crates/render/instruments/samples/mixer` | ABI 2、资源/config prepare、增量实例移交、采样视图与路由执行                          |
| `crates/napi/wasm`                        | 协议 2 控制桥，错误与数据边界，无 JS 音频执行                                         |
| `apps/preview`                            | 升级为 GPUI DAW：编辑控制器、视图投影、插件管理器/面板、事务状态                      |
| `schemas/include/fixtures`                | 全部新协议、ABI header、TS/Rust 生成结果与 conformance                                |

core 从当前 native playback 副作用中解耦。播放/导出通过 runtime session facade，
例如 `createSession(project, options)`；Project 只组织 authoring，不在 factory 求值时
隐式打开 engine。统一 `oxitone` 入口继续提供方便 API，不形成 core → sdk 循环依赖。
SDK 构造插件配置也不 dlopen；依赖解析与 native 验证按 session/control 阶段执行。

## 2. 需要定义的协议产物

在实施第一个 GUI edit 之前，先给以下类型建 JSON schema、TS types 与 Rust serde 镜像：

- `AuthoringDocument`：版本、时钟、seed、definitions/configs/instances、typed sources、
  tracks/placements、explicit order、bindings、资源与依赖描述；没有原始 JS closure。
- `EngineProject`：有限 sources 的已解析输出与 musical origin、源窗口、automation DAG、
  config/resources/typed targets，Rust 编译所需数据；不包含 TS AST/源码文本。
- `ViewProjection`：已接受的 source/entity/output handles、生成层级、能力、resolved slices/
  regions、诊断与 active graph generation；没有 DSP 指针。
- `EditRequest/Result`、`DocumentEvent`、`SaveTransaction` 与 `RecoveryJournal`：版本、
  request/transaction IDs、read/write sets、前置条件、结果和稳定错误码。
- `PluginDefinition/InstallManifest/Capability/ConfigState/ResourceSlot` 与 UI protocol 2。
- `RuntimeCommand/RuntimeAck`：typed target、generation、frame、transport/试听/graph swap。

只有 source 文档服务需要文本 hash/AST anchors。EngineProject 的 ID 是执行引用，
不能成为文件位置、随机种子或业务排序。PluginDefinition 的准确来源进入依赖解析，
EngineProject 只接收已验证执行能力与资源 handles。

稳定错误至少包括 SourceChanged、EditTargetMissing、EditTargetAmbiguous、EditScopeConflict、
EditNotRepresentable、DraftInvalid、SaveConflict、RecoveryConflict、GraphRejected、
PluginMissing、PluginBinaryConflict、PluginCapabilityUnsupported、PluginConfigInvalid、
PluginMigrationFailed、ParameterTargetInvalid、ResourceChanged、BudgetExceeded。
错误带 affected source/range/target、是否可重试和保留的 accepted revision，不能要求解析字符串。

## 3. 分阶段执行；每阶段都有可演示的闭环

### P0：目标契约与全仓迁移基线

把本设计同步到 `.agents/docs/01/02/04/06/07/08/09/11`、路线图与 oxitone-guard，
更新只读 Preview 和“保持旧协议兼容”的旧目标。当前用户已授权发布前重定义，不需要
为了这些旧定义另设批准流程。建立新 schema/header/fixture，记录旧示例迁移清单。
出口：文档、类型与生成器可以对拍，旧协议明确拒绝，不留半套 ABI 混用。

### P1：纯 authoring 与生成编辑语义

实现 Source/Config/Instance/typed Target、chord/arp、集合 edit、Arrangement/window、
24 个 automation source 与 range edit；定义并实现新 random origin/context。
先用纯 TS/Rust/offline fixture 验证 edit 与来源保持；不依赖 GPUI 才能使用新能力。
出口：生成式音乐的局部变化、删除留空拍、共享引用隔离与包装不改随机结果均可验证。

### P2：源码 writer 与 Document Service

按 [07](07-mvvm-and-localization.md) 增加模块 import/export planner、源码所有权检查、
本地化候选与确认、外部代码缓冲区桥接；仅文件 watch 不算未保存代码实时同步验收。

实现多文件源码加载、AST/来源、语义命令、最小补丁、Undo、冲突、save journal 与 fresh load。
以可编程 document session 演示 chord → arp → repeat 的局部修改后保存、删缓存、重开。
加入外部 factory 返回配置/Pattern.generate 边界，以及不透明副作用的明确诊断。
出口：所有主要 edit 有可读 TS 输出，round-trip 性质成立，多轮编辑不会包装无限增长。

### P3：Rust 执行与外部插件 ABI 2

迁移 scheduler/MIDI、random、automation DAG/ranges、typed parameter queue、source windows。
实现 ABI 2 参数/资源/state prepare 与配置迁移，迁移真实鼓机/C gain fixture，新增外部
采样资源与 structured/opaque config fixture。打通 definition resolver 与真实离线导出。
出口：插件资源/state 真实到达 C 边界，同名参数独立，native/Wasm 共享适用的执行语义。

### P4：GPUI 完整编辑工作区与插件管理器

接入 Playlist/Piano/Mixer/Automation/Sample 的语义手势与 pending/accepted 投影。
实现实例/定义/轮次范围、code navigation/diff、所有视图 Undo/Save，以及通用/声明式
插件面板。插件管理器提供完整目录、验证、安装/版本任务、添加替换、缺失修复与引用跳转。
出口：外部音源与效果器也能完成图形创建/编辑/保存 TS/关闭重开，不限于内置 demo。

### P5：增量播放、原生 UI、恢复与发布门槛

完成参数连续试听、受控实例移交、graph active ack、native UI companion gesture/config
回写，处理取消、过期事件、多窗口、安装失败与崩溃恢复。迁移示例、API docs 和测试基线。
出口：下表验收全部满足，性能基线归档；有明确平台能力，首次发布只包含通过的 adapter。

各阶段是实现顺序，不削减目标范围。完整版本不能在参数演示通过后就宣称全部双向绑定完成。
依赖链是 `schema/authoring → writer/session 与 runtime/plugin → GPUI → integrated release`。

## 4. 必过行为矩阵

| 领域        | 最低验收                                                                                       |
| ----------- | ---------------------------------------------------------------------------------------------- |
| chord       | 六 quality、open/inversion、单音 pitch/start/duration/velocity、增删、两个截顶同音不误选       |
| arp         | 四 order、多 octave、重复 input、单步修改、删除留空拍、插音不重算旧力度、seed 重开一致         |
| 组合        | chord → arp → concat/repeat → transpose → placement，四种作用层次与跨文件共享引用              |
| 源码        | literal、变量、alias、shared options、async factory、纯返回 helper、不可隔离副作用诊断         |
| MVVM / 模块 | 双向 dirty buffer、防回环与冲突；import/export 别名/遮蔽/条件导出/副作用/循环                  |
| 本地化      | npm 编曲/效果器组合拆散提示、最小作用范围、绑定/资源/随机保留、依赖源码 hash 不变              |
| edit 归约   | 同目标重复拖动不累加包装；insert/remove 抵消；expect 失败不猜目标；无操作字节不变              |
| 结构        | Clip 拆分/移动/替换/跨 Track、窗口截断、共享 Pattern 派生、Loop 单轮编辑与长音延续             |
| automation  | 全 24 入口、不可逆条件、source/lane range、边界/交叉渐变、loop/hold/order、tempo 限制          |
| 随机        | 包装/移动/重排/source span/ID 变化不改变未编辑事件；复制共享 seed、variation 独立、restart     |
| 采样        | frame/beat、trim/fit/tempoSync、Sample Split 相位、Slicer grid/onset 单片 edit 与 trigger 迁移 |
| 分区        | grand/soft、多层/单区修改、资源/defaults 保留、键力度不重叠                                    |
| 插件        | 两个同版本实例只改一个；host/plugin 同名参数；资源与 state 从 TS 经真实 ABI 2 到 DSP           |
| 面板        | 无 UI 仍可编辑；坏布局回退；原生 companion gesture、拒绝/取消、旧 generation 不误写            |
| 重排替换    | insert 对象绑定不依赖 index；升级参数/state 迁移失败保留全部源码/automation                    |
| 管理器      | 未使用内置/外部可见、缺依赖、精确版本、多插件包、平台/hash/签名、安装失败/重新定位             |
| 保存        | 多文件崩溃点注入、恢复第三方 hash 冲突、autosave、磁盘满/权限/rename失败、备份可恢复           |
| 并发        | 外部格式化/重命名/插入/删除/重复实例；三方文本可合并但语义冲突不误写                           |
| 进程        | runner/helper/UI 崩溃、断线重连、request 幂等、queue full/过期 candidate 与 late ack           |
| 重开        | 删全部可丢弃缓存后，仅源码/locks/assets 重建；CLI/GPUI/offline 采用同一配置与音乐              |
| 音频        | 64/128/256 block、原点/loop边界、有效 tempo/Track clock、WAV/MIDI 事件与随机对拍               |

特别验证 literal 重复事件的默认随机相关性是已定义行为，不能为了测试通过隐藏生成 UUID。
对 opaque state 验证字节保真/资源引用与恢复，不伪造能逐字段编辑私有二进制的测试。

## 5. 检查与性能

实现变更按仓库运行 `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、
`cargo test --workspace` 和聚焦 benchmark；TS 增加 narrow authoring/writer/protocol 集成测试。
音频全部用 offline PCM/WAV、MIDI、simulated sink；浏览器 no-device sink 不可用则失败。
不得用打开系统输出作为测试 fallback。native UI/GPUI 物理交互单独人工验收，不替代语义测试。

在固定 Apple Silicon/macOS、48 kHz、128 frames、release 基线记录下列目标；这是验收预算，
不是已测结果或跨硬件保证：

| 项目            | 目标与测量范围                                                                |
| --------------- | ----------------------------------------------------------------------------- |
| GPUI 手势投影   | 60 Hz；局部 frame p95 ≤16.7 ms，重建不阻塞 UI                                 |
| 连续参数试听    | UI 发命令到 enqueue 的 p95 ≤30 ms；另外报告 ring/device audible latency       |
| 小工程局部 edit | warm build、无资产 I/O，源码到候选接受 p95 ≤300 ms                            |
| 大工程          | 100k notes/1k placements/100 lanes，报告索引/峰值内存/局部编辑 p50/p95/p99    |
| 插件目录        | 1k metadata 条目检索/滚动不执行代码、不阻塞 UI；验证/安装另测                 |
| 原生 realtime   | 保持 05 的场景预算，记录 callback/worker p95/p99/xrun；开启编辑与管理任务对比 |
| 随机/range      | 节点共享、嵌套 edit 和密集 range 的编译/每 block 成本，含超预算诊断           |

重建不可达目标时优先做增量索引、模块缓存、参数命令与后台 prepare，不能降低校验或
只更新图形来“达标”。测不到的 callback/设备数据记未测，不用离线平均数冒充。

## 6. 设计完成与实现完成的区别

设计阶段的产物是统一目标及实施/验收清单。当前已有 Pattern source DAG/edit、完整工程
Document Service、多文件 Save/恢复、editor IPC、GPUI 音符/source automation 编辑，以及
插件目录/验证、初始配置回写和串联 rack 拆散。实际 API、测试范围及限制见
[15](../../docs/15-source-authoring.md)、[18](../../docs/18-project-daw.md)、[19](../../docs/19-plugin-configuration-source.md)。
实例/typed automation 与 GPUI 排序及无 ID 源码回写见 [20](../../docs/20-plugin-instances.md)。
VS Code linked TypeScript buffers、版本条件、自动重连与 journal Save 见
[21](../../docs/21-editor-session.md)；完整 language service 仍待接入。
P0–P5 均不能标记全部完成：engine 已迁移 snapshot 1.1，ABI 仍为 1；typed runtime commands、
random origin、lane range/窗口、外部资源与 state、安装/升级管理和增量播放仍待完整迁移。
先前的 45 个高阶 authoring、3 个动态 C 插件与 1 个布局注册测试只说明旧实现基线，
不能作为 ABI 2、完整 Document Service 或 GPUI 编辑已经通过验收的证据。

设计已确定采用 source graph + 输出 edit、统一 Document Service、新随机模型、typed
实例/参数、ABI 2 与插件管理器，不再把旧草案中的兼容分支当成必须保留的产品要求。
剩余工作是实现、针对不可证明的程序边界给出诊断，以及用上述门槛实测验证，而非继续
依赖每次图形操作去猜测作者想怎样重写任意高阶函数。
