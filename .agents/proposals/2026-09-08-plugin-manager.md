# 外部插件兼容与 GPUI 插件管理器

已被 [完整代码 / DAW 架构](../designs/source-daw/README.md) 及其插件规格取代。
目标采用新 ABI 2，不再要求长期兼容旧 ABI 1；下文保留现状调查与原始需求记录。

状态：讨论草案，2026-09-08；是 [源码 DAW 方案](2026-09-08-source-backed-daw.md) 的必需组成。
本次将外部插件纳入设计和验收范围，并新增插件管理器需求；没有实现该界面或新协议。
实施时同步修订 04/08/09/11 的正式契约，当前只读 Preview 与现有 ABI 仍按原规范工作。

## 1. 兼容目标与现状

图形编辑、保存 TS、CLI 执行、重新打开和离线导出必须共用插件解析与注册规则。
外部插件不要求提供源码或 AST 逆变换器；只要能力契约满足，就能通过通用面板编辑。
自定义 UI 是增强项，不是插件可用或可编辑的必要条件。

这里的已支持外部格式是 Oxitone C ABI v1 的 Instrument/Effect 动态库。
VST3/AU、原生 UI companion、动态 Wasm 插件不能因为加入管理器就标为支持。
浏览器无法加载 macOS dylib；现有 Wasm 仅静态集成插件，平台能力要单独列明。

| 能力                            | 当前实现证据                                                          | 接入源码 DAW 的要求                                                  |
| ------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------- |
| 外部音源/效果器注册             | `Project.registerPlugin`、native `registerPlugin`、Rust `load_plugin` | 保存显式注册来源、准确版本与实际 hash，所有执行入口一致              |
| 参数描述与初始值                | manifest 与 C descriptor 有序参数表必须相符                           | 名称、单位、范围、默认值、mapping/rate/smoothing 取已验证 descriptor |
| 参数事件与 automation           | C ABI 使用参数索引传物理值，宿主提供归一化 automation                 | 通用控件发同一语义命令，Rust 做最终校验，不让 UI 决定 DSP 参数语义   |
| host mix/bypass、sidechain、PDC | 按宿主/descriptor 能力处理                                            | 区分 host 参数和插件参数，结构改动重新校验路由和延迟                 |
| 声明式 GPUI 面板                | `registerPluginUi` + uiVersion 1.0，内外插件共用                      | 扩展为可编辑时复用统一命令路径；坏面板回退通用面板                   |
| 注册与实例故障诊断              | hash/签名/ABI 错误，process fault latch/count                         | 管理器展示准确来源与诊断；目前计数按插件版本聚合，不伪装成逐实例 CPU |
| 插件注册信息持久化              | 当前不进入 ProjectSnapshot/Project.save 的 manifest                   | TS 保存必须写注册代码/import，不能仅保存在管理器数据库               |
| resources / structured state    | 内置插件有支持；C ABI v1 没有传输接口                                 | 外部插件不能宣称支持，需显式能力检查与后续 ABI 扩展                  |
| 原生 UI / 任意内部状态回读      | 当前没有对应 C ABI 接口                                               | 不能从插件窗口直接偷改 DSP，再假装已保存到 TS                        |

代码依据：`packages/core/src/project-playback.ts`、`packages/protocol/src/plugin.ts`、
`parameter.ts`、`plugin-ui.ts`、`include/oxitone_plugin.h`、`crates/render/src/plugins/`、
`crates/render/src/build_plugins.rs`、`apps/preview/src/plugin_catalog.rs` 和 `plugin_details.rs`。

重要缺口：当前 wire 能携带 state/resources，并不表示 CInstance 收到了它们。
非内置 create 分支仅传 HostContext 和参数事件。新的能力校验应在此类配置不受支持时
拒绝候选并给出明确路径，不能保存成功却在外部插件中静默失效；该校验本次尚未实现。

## 2. 外部插件怎样参加无显式实例 ID 的源码编辑

厂商分配的 pluginId/parameterId 是 ABI 名称，继续从插件包/manifest 获取。
用户不需要给自己的每个插件实例手写 ID；注册时用 manifest 身份，实例定位使用
源码会话 handle、owner、放置/insert 来源与 source revision。

外部包可以返回普通 `InstrumentRef` / `EffectRef`，也可以提供自己的 TS factory。
对 factory 不要求理解其私有实现；如果没有已知参数映射，局部修改放在 factory 的输出侧：

```ts
// 示意：vendorSynth 返回 InstrumentRef，且 descriptor 声明 cutoffHz 参数。
const source = vendorSynth({ preset: "Warm" });
const instrument = {
  ...source,
  parameters: { ...source.parameters, cutoffHz: 900 },
};
```

这保留 factory 的其他返回字段，也不改 node_modules。同一 source 用在多个 Channel 时，
只给选中实例使用的引用加局部变体。关闭 GPUI 后，正常执行入口仍会应用这个物理参数值。
factory 结果不可序列化、参数依赖不透明副作用或涉及 ABI 不支持的状态时，沿用来源能力
限制，不能把任意插件私有对象强行打印成 TS。

参数编辑必须区分 source 显式值、descriptor 默认值、automation 写入与试听临时值。
“恢复默认”需要定义为删除显式设置或固定为当前版本默认值；它们在升级后语义不同。
readout 保持只读；automation=false 的参数不提供写 lane，普通初始参数编辑仍按能力处理。
enum 标签来自合法布局选项或未来 descriptor 扩展；当前 descriptor 只有数值域，
不能凭单位 enum 编造厂商枚举名称。所有控件保留原值的精度及合法物理单位。

拖动过程可通过受控参数命令试听，提交/保存仍重新执行候选 TS 并校验图。
现有 Preview 没有插件参数 setter IPC，不能把 native.setParameter 的存在当成 UI 已接通。
新命令须带实例 handle、参数路径、graph/source revision 和事务 ID；参数索引只在 native
已接受的具体 descriptor 下解析，不把旧索引套到升级后的插件。

### 2.1 必须补齐宿主与插件参数的明确寻址

当前 `crates/graph/src/compile/bindings.rs` 和 `crates/render/src/params.rs` 优先解析
Channel 的 level/pan/mute/swing，`insert.*` 也有宿主路径含义。外部音源可合法声明同名
参数，其初始 `instrument.parameters` 配置仍可写入，但直接沿用裸 parameterId 的实时
命令/automation 会命中宿主或被误解。Preview 详情目前已对同名参数清除误导的 automation
标记；它没有解决编辑寻址。兼容验收不能只使用没有名称冲突的 volume/gain fixture。

建议新增明确 target kind（Channel 参数、音源参数、insert host 参数、insert 插件参数），
将厂商 parameterId 作为原样字符串字段，独立于 owner/slot；不要用另一个容易冲突的
字符串前缀掩盖问题。GUI command、TS authoring binding、wire、Rust automation compiler、
实时参数 resolver 和回显都要一致。新增 TS 入口可由 Channel 对象提供，不要求用户填写实例 ID。
旧裸路径维持原优先级兼容；协议版本与迁移按 04 定义，不把新行为塞进旧路径默默改变含义。
在该能力完成前，同名插件参数只能通过明确的 source 初始配置改写/换图，不能发可能改错
目标的试听或 lane 命令；管理器和面板如实显示能力限制。

## 3. 与高阶生成、复制、替换和重排兼容

| 场景                                 | 必须满足的编辑语义                                                               |
| ------------------------------------ | -------------------------------------------------------------------------------- |
| loop/map 生成多个外部音源 Channel    | 某实例参数变化只派生该输出配置，其他实例仍跟随原 factory                         |
| 一个效果器配置复用于多条链           | 编辑选中 insert 的引用，不能把同 pluginId/version 的全部实例一起改掉             |
| 外部包提供生成式 Note/Pattern helper | 按高阶函数审计的输出边界处理；不需要该 npm 包公开源码                            |
| 复制外部插件实例                     | 复制可保存的 authoring 配置、host 设置和明确绑定；不复制运行中 DSP 内存          |
| insert 重排                          | 迁移相应 `insert.<index>.*` automation、窗口选择和待提交事务；不能让它们跟错槽位 |
| 替换音源/效果器                      | 选定范围内重建引用；检查参数/能力/资源/sidechain/automation，再提交候选          |
| 效果器 bypass                        | 保留实例配置与链位置；不等同删除/卸载，缺库时不能假定 bypass 能免除加载          |
| 外部插件产生内部随机声部/私有琶音    | 宿主只编辑公开参数；未暴露为 authoring 数据的音符不能假装可逐音反写              |

同名参数也不证明两个插件的单位、mapping、范围和含义相同。跨插件替换默认使用新插件
默认配置，只有明确兼容的映射才迁移参数；未映射 automation 作为待解决项目保留，
不能悄悄删掉或钳到新范围。跨版本也要验证，不能只根据 semver 次版本号自动判断无损。

所有实例变化都走“语义命令 → 源码草稿 → 执行/校验 → 候选图 → 保存 TS”。
GUI、通用插件面板、自定义 GPUI 面板与未来原生 UI adapter 都不能另设绕过源码的保存路径。

## 4. GPUI 插件管理器：产品范围

新增独立的 Plugins 入口，可从主工作区打开，也可从音源选择器/insert 添加入口进入。
管理器使用共享目录状态，当前选择的 Channel/插槽作为“添加到工程”的上下文。
浏览列表、筛选、查看详情不触发播放、工程保存或 DSP 实例创建。

建议采用左侧筛选、中央列表、右侧详情与使用位置的布局：

| 区域     | 用户看到的内容                                                           |
| -------- | ------------------------------------------------------------------------ |
| 分类     | 全部、音源、效果器、工程使用、缺失/不可用；可筛选内置与外部              |
| 搜索     | 插件名称、包名、厂商（元数据存在时）与插件标识                           |
| 列表     | 名称、类型、版本、本机可用状态、工程使用数量；正在执行的管理任务有进度   |
| 详情     | 支持的平台、参数、面板可用性、来源、版本；技术细节折叠显示 ABI/hash/路径 |
| 工程引用 | Channel/Bus/Master 与槽位，可跳转；区分草稿和当前播放版本的引用          |
| 问题     | 缺依赖、库缺失、架构不匹配、ABI/签名/hash/descriptor 错误及重新验证入口  |

插件名称/厂商不是当前 manifest 的必填字段，不能从 pluginId 猜造。缺省用现有
pluginId 展示；未来安装元数据提供可选 displayName/vendor/tags，不能改变音频身份。

### 4.1 核心操作

- 查看全部内置插件和已发现外部插件，包含尚未挂载到工程的版本。
- 接入已安装的插件 npm 包；导入用户选择的 manifest 与本地动态库；重新定位缺失来源。
- 显式验证选定插件；显示参数、平台、面板、错误和在当前工程中的使用位置。
- 将音源加入新 Channel 或替换选中 Channel 的音源；将效果器插入 Channel/Bus/Master。
- 打开已有实例面板；复制、替换、移除选定实例，移除不自动卸载插件包。
- 移除不再需要的工程注册；有实例、预设、生成分支等可能仍引用时不自动清理。
- 管理版本与重新定位；升级有独立候选验证和回退，不能直接覆盖已加载 dylib。
- 管理本机目录记录。隐藏/忘记记录、取消工程注册、删除实例、卸载包是不同操作。

安装/卸载 npm 包也属于管理器的后续交付范围；首个可用版本先覆盖已安装包和本地库。
联网安装必须由显式 Install/Update 操作发起，显示准确包名/版本和任务状态，使用项目
package manager 与 lockfile；首次播放、打开工程和刷新列表不能隐式下载缺失二进制。
项目 package.json/lockfile 的修改是明确依赖管理事务，与 TS 编辑同样需要可恢复记录；
不能把它误作只改单个 TS 文件。包脚本执行与失败恢复须按实际 package manager 设计。
非本工具拥有的本地库默认只移除引用/目录记录，不删除用户原文件。

### 4.2 明确区分四类状态

1. 本机目录：可找到哪些包/库/版本，包含尚未加载或尚未验证的条目。
2. 工程声明：TS 注册了哪些插件和布局，源码中有哪些引用。
3. 已接受快照：此 revision 实际实例化哪些插件；引用数不包含未执行分支。
4. 当前运行：哪些图/实例仍持有库，以及可取得的故障诊断。

它们是不同维度，不压成一个“已安装/已启用”开关。目录中存在不代表已通过 native
校验；已验证不代表工程已注册；草稿删除最后实例也不代表旧播放图已经释放库。
缺失插件保留原 TS/参数/automation，继续显示问题；已有会话保留最后合法图。
全新打开的工程若没有合法图则保持停止；不能自动换成别的插件，或把静音预览当作成功编译。

## 5. 发现、验证和持久化

当前只加载显式路径、不扫目录。新管理器默认从工程注册、已安装包的声明式元数据、
用户显式导入记录建立目录；不扫描整个文件系统，也不按 AU/VST 的位置搜 dylib。
若以后支持指定目录扫描，必须是显式加入的目录、受限任务与单独契约变更。

现有 npm 包约定是导出 manifest、sha256、pluginPath()，尚没有统一的可发现 package.json
标记或数据文件入口。新增静态 discovery 元数据需版本化，列出 manifest、平台库、可选 UI
描述与宿主要求；旧包仍可经用户选择显式接入，不把新 metadata 变成加载既有插件的强制要求。
发现阶段不为了搜索而 import 所有 npm 包；执行 pluginPath/私有解析器与 native 验证属于
用户选择接入后的独立任务。任意 TS 模块或 dylib 初始化器都不能视为纯元数据读取。

native 验证使用可终止的控制侧 helper，输出有界、拥有所有权的 descriptor 与诊断。
崩溃/超时令该候选不可用，不阻塞 GPUI 或当前播放；这只隔离验证进程的故障，
不表示运行期音频插件已获沙箱保护，也不防止该 helper 中代码访问文件或网络。
浏览时不调用 create/prepare/process；候选工程验证可能 create/prepare，仍不得开启设备。

工程相关持久化：

- 在原注册位置或可读的本地 TS 模块保存 `project.registerPlugin(...)` 与可选
  `registerPluginUi(...)`，由入口正常 import/调用。不能只写管理器私有数据库。
- npm 来源优先保留包 import 与平台路径解析函数；本地库使用工程相对 URL/明确路径。
  全新机器的系统绝对路径需要重新定位，不宣称复制 TS 即包含 dylib 和依赖。
- 使用准确版本与 expectedHash 校验二进制；跨架构 hash 由正常导入的 per-platform
  数据提供，不能把本机 arm64 hash 锁给 x64。包 lockfile 与库 hash 是不同层的校验。
- 注册必须先于该工程的 native compile/play；源码保存后只有 CLI 也能正常注册并运行。
  对不透明 factory 或无法安全插入的初始化顺序，先形成可审查重构，不重复执行 factory。
- UI 布局记录单独版本/hash，不把纯布局变化误作音乐变化；UI-only 更新继续复用音频实例。

本机目录缓存可保存发现结果、最近位置和验证指纹，删除后可重建；不能持有唯一的
工程参数、注册策略、二进制 hash 或版本迁移结果。安装记录/依赖锁不属于可随意丢弃的缓存。

## 6. 版本、能力和信任策略

同 pluginId/version 不同 hash 是冲突，不能就地覆盖或通过忽略 expectedHash 解决。
新版本与旧版本可有独立候选记录；选择更新工程时验证具体实例、布局、默认值、参数及
automation 是否兼容，再更新 TS 与依赖锁。旧文件在旧图释放前保持不变，卸载也要考虑
同机其他进程和工程使用，不能仅凭当前窗口引用数断言可以安全删除。

同一编排中的多个版本须按精确版本独立解析；不要把 npm package version 与 pluginVersion
假设为相同。一包也可能提供多个插件，插件管理器的数据模型必须允许这种关系。
npm 无法原样并存的包版本需显式别名/隔离解析方案，不在工程里悄悄重定向到最新版本。

实际签名策略有入口差异：Preview 默认 SignedOnly，native 的省略策略在 debug 为 Any、
release 为 SignedOnly。新 resolver/manager 必须把工程策略显式传到所有验证与执行入口，
不能开发时通过而 GPUI 失败。当前 allowPlugins 是工程/引擎级，不能假装已有每库授权。
管理器不会为加载一个 unsigned 开发库自动把整个工程改成 any；已有显式工程策略沿用。
不绕过签名/hash 错误，不自动清理 quarantine 或给用户库重新签名。

ABI v1 扩展的目标要单独明确：

| 后续能力              | 必需契约                                                                    |
| --------------------- | --------------------------------------------------------------------------- |
| 外部采样资源          | 控制侧声明、资源寻址/hash、准备数据与所有权、版本化传递接口                 |
| 外部 structured state | 可序列化配置/schema/version、验证与迁移、控制侧恢复；不保存 DSP 内存        |
| 插件内部 UI 编辑      | gesture 开始/变更/结束通过宿主事务；有能力时提交 state/参数，最终由源码接受 |
| 原生 UI companion     | 独立于 audio ABI，按 11 的生命周期扩展；缺 companion 仍可用通用面板         |
| 实时有效值反馈        | 有界遥测、graph generation、来源/默认/有效值区分，不能从 UI 推测            |

兼容旧 C ABI v1 的参数型插件是必过门槛。扩展遵循 ABI record size/版本协商，不能只在
TS InstrumentRef 上增加字段就宣称外部插件支持。VST3/AU 需要额外 adapter 与各自状态/UI
协议，管理器可预留 format 字段，但不能承诺当前动态库加载器能处理这些格式。

## 7. 分层与需要补齐的接口

- GPUI：管理器与实例面板、目录/引用视图模型、异步任务和状态呈现；不读取 DSP 指针。
- Node 控制侧：工程包解析、安装任务、源码编辑事务、注册代码生成和源版本冲突处理。
- Rust 控制侧/helper：静态元数据/平台检查、hash/签名/ABI/descriptor 校验、候选图编译。
- Rust realtime：继续只消费已准备的图与有界事件；不运行扫描、包管理、源码或 UI 代码。

目录 API 独立于播放图：当前 `plugin_catalog::collect` 只收集 snapshot 引用的插件，
拿它做管理器会漏掉未使用内置插件、已注册未使用外部插件，以及缺失候选。
现有 PluginInfo 也不涵盖安装来源/平台/验证状态，需要新增版本化管理元数据。

IPC 需要目录查询/更新、任务进度/取消、验证结果、编辑意图/结果与独立请求 ID。
目录/验证 revision 与 source/graph revision 分开，不能用 snapshot hash 去重丢掉目录变化。
注册变化不应天然使未使用插件重置整首歌；当前 audio_key 包含全部注册，需要基于已接受
图实际用到的依赖精确判断，并保留 native 权威验证，不能直接删除 fingerprint 字段取巧。
不同入口的 reserved ID/重复注册规则也需统一，测试精确 ID/version/hash 幂等与冲突。

## 8. 验收与交付顺序

1. 目录与管理器只读基础：所有内置、工程外部注册/引用、缺失条目、搜索/跳转/诊断；
   开关管理器不创建 DSP、不播放、不改源码，布局或目录刷新不重置播放。
2. 已安装包/本地库的显式接入、验证、注册写回，加上外部实例添加/参数编辑/替换/移除。
   完成明确参数寻址，从保存 TS 在新进程重建；删除目录缓存后依然成功，源码外部修改能恢复。
3. 版本切换、实例复制、insert 重排与 automation、包安装/卸载事务和缺失来源修复。
   同 ID/version 异 hash、错误架构、旧 ABI、加载超时、坏布局和 prepare 失败均保留旧图。
4. 在扩展 ABI 完成后单独验收 resources/state/native UI；当前不支持的能力先明确拒绝，
   不能用内置 Sampler/Slicer 测试替代外部 C ABI 传输验证。

真实 fixture 必须覆盖外部音源 `example.drums` 和外部 C 效果器 `fixture.gain`，以及
有/无自定义面板；检查初始参数、host mix/bypass、automation、共享配置局部编辑、
watch 保存回读、缺依赖与重新定位、离线 PCM/WAV、模拟播放和故障诊断。
创建两个同版本实例再编辑其中一个，必须证明另一个配置不变；重排后旧 UI gesture 不得
落到新槽位。同名 level/pan、包含点号/insert 前缀的合法厂商参数须覆盖宿主/插件独立编辑。
保存成功至少要求 fresh TS → 注册 → compile/render 可重现 authoring 配置。
插件若有自己的不可复现 DSP 行为，不把“配置可复现”夸大为所有第三方 WAV 必然逐位相同。

本次已运行 `pnpm --filter @oxitone/native exec vitest run test/plugins.test.ts`：3 个测试通过，
使用真实本地编译的 C 动态库，覆盖注册隔离/hash 冲突、参数/automation 与离线 WAV/故障静音。
另运行 core 的 `test/plugin-ui.test.ts`：1 个测试通过，覆盖版本化布局注册与快照隔离。
测试均不打开音频设备；它们证明现有外部插件基础行为，不证明管理器与源码编辑已实现。
