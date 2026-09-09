# 外部插件 ABI 2、配置编辑与 GPUI 插件管理器

## 1. 统一插件对象与能力

内置与外部插件共享 PluginDefinition → PluginConfig → PluginInstance 模型。
Definition 是准确版本的描述与安装定位，Config 是可序列化创作配置，Instance 是 graph 中
独立引用的音源/效果器。实例 handle 自动产生；用户通过对象引用操作，无须填写实例 ID。
pluginId/parameterId 是厂商 ABI 名称，和用户实体 ID 的限制不同。

新版 manifest 为独立静态文件，包含：formatVersion、pluginId/version、displayName/vendor、
kind、平台/架构库入口与 hash、ABI/minHost、audio layouts、参数表、资源 slots、state schema、
capabilities、可选 UI 描述、可选迁移入口与许可证/来源信息。
一个包可以提供多个插件；package version 与 pluginVersion 独立，不能假设总相同。
目录浏览读静态文件，不先执行 package 的 JS 入口或 dlopen 每个库。

capabilities 至少区分 note input、sidechain、tail/latency、parameter automation、
prepared resources、structured/opaque config state、configuration migration、UI 类型、
deterministic render、平台支持。没有某能力就不提供相关编辑入口，并在候选验证时拒绝
非法数据；不允许 wire 保存了字段而插件实际忽略它。

## 2. 参数与资源是完整的创作契约

ParameterSpec 增加明确的 value kind、enum choices/step、mapping、单位、范围/default、
read/write/automation 能力、smoothing 和 updateMode（event 或 prepare）。prepare-only
参数不能发 realtime setter，必须走配置候选。参数 ID 原样字符串，不将点号当嵌套路径解析。
宿主 channel、plugin instrument、plugin effect、effectHost 分 target kind；level/pan/mix/
bypass 即使重名也能独立编辑。所有自定义面板都走相同 typed target。

资源 slots 声明类型（sample PCM、IR、wavetable、opaque immutable bytes 等）、必需性、
声道/格式约束与使用时机。TS Config 保存正常 AssetRef，宿主控制侧查找/hash/解码/转换；
插件 prepare 接收只读资源 views/handles，process 不读文件或解析路径。
资源 lease 由实例持有到 dispose 之后，更新资源生成新 lease，不能替换旧实例仍借用的 buffer。
PCM 不跨 JS；UI 获取有界元数据/显示缩略，不获取运行实例的可写音频指针。

## 3. ABI 2 的控制与实时边界

新 C 入口 `oxitone_plugin_entry_v2`，结构都有 abiMajor/abiMinor/structSize 前缀、明确容量
与返回码。ABI 2 用当前仓库整体迁移，不保留音频运行时兼容 ABI 1 的必需分支。
C 边界不用 Rust String/Vec/trait object，不传 JS 对象；指针所有权与有效期逐字段定义。
Note event 增加宿主分配的 eventToken，note-off 精确对应同次 note-on，允许同音高重叠声部。
token 只在运行事件生命周期内有效，不写入用户 TS，也不参与随机 seed。所有事件带
frameOffset、明确 kind 与有效 count/capacity；同帧优先级和溢出错误与引擎调度协议一致。

| 调用族 | 线程 / 数据 |
| --- | --- |
| describe | 验证 helper/控制侧；复制静态 descriptor，与 manifest 核对 |
| validateConfig / migrateConfig | 控制侧；只读配置与有界输出，错误保留原配置 |
| create(config, resourceViews, host) | 控制侧；实例独占，配置数据必须复制或由约定 lease 持有 |
| prepare(rate, maxBlock, busLayout) | 控制侧；分配 DSP/事件缓冲，发布固定 latency/tail 能力 |
| process(audio, notes, parameters) | realtime；只用预分配数据、物理参数与 frame offsets |
| reset(reason, positionContext) | realtime-safe；原因明确，不隐式改创作配置 |
| getConfiguration | 控制侧配置对象/安全快照；不允许并发读取运行 DSP 内存 |
| dispose | 控制侧；先销毁实例，再释放资源与动态库 |

配置状态和运行状态分离：TS 中的 parameters/resources/state 决定可重开配置；voice、delay
buffer、LFO 当前相位等是运行状态，不作为普通 Save 的内容。getConfiguration 不成为每次
保存必需的 DSP 抓取接口；正常状态应早已由宿主的配置事务持有。
需要运行态快照的插件须额外协商有界发布或停机捕获，不在音频回调调用序列化。

structured state 用版本化 JSON/CBOR schema 表达，可通用字段编辑或自定义面板编辑。
opaque config 用声明格式/version 的 immutable bytes，作为显式内容寻址资产，TS 引用它；
宿主可以保存/恢复但不假装能理解其内部字段。state 不可序列化的插件不达到工程保存门槛。
小型编辑状态写 TS 可读数据，大型 bytes 与 sample 一样是可移植工程资产。

插件校验若需要规范化 config，必须返回明确的 canonical config，由宿主反映到源码事务
并再次核对，不能私下改变配置而保存另一套值。能力/schema/参数默认值属于准确 pluginVersion，
升级必须通过迁移或显式映射，不能无声
钳值、丢 state、清 automation。对迁移函数也使用可终止 helper，它不是安全沙箱。
原生插件运行仍属受信任代码；返回错误/NaN fault latch 可处理，任意崩溃/无限循环的运行期
隔离需要独立宿主进程，不以 ABI 校验或 hash 宣称已经实现。

## 4. 自定义 UI 与宿主编辑事务

UI protocol 2 支持声明式 GPUI 控件、结构化 state 控件和独立 native UI companion。
声明式面板与通用面板都绑定 typed instance parameter/state target，不直接持有 DSP 实例。
native companion 只接收配置/描述的复制和宿主 UI context，通过 Begin/Update/End/Cancel
gesture 发送配置修改意图，宿主生成源码事务并校验后回传接受版本。

面板必须能显示 pending/accepted/rejected/source revision；宿主拒绝后回滚该手势，
不能让面板内部状态成为唯一真相。第三方 UI 不能偷偷改 DSP、读私有 state 再绕过 TS Save。
完整私有状态变化可发送有界新配置或 schema patch，由宿主保存为 TS 或显式资产。
纯 UI 偏好不写音乐配置；缺失/损坏 UI 单独回退通用面板，不阻止合法音频插件工作。

窗口按 instance handle + generation 绑定；重排后跟随实例，替换后旧 gesture 拒绝。
旧 UI 的异步回调即使参数名相同也不能套到新版本实例。UI 的生命周期和 DSP 独立，
关闭面板不销毁仍在播放的插件；打开面板不创建第二个音频实例。
第三方 UI 不支持结构化回写契约时，只能提供明确只读 UI 或宿主参数面板。

## 5. 外部 factory、高阶生成与预设

外部编曲与效果器组合的本地化流程见 [07](07-mvvm-and-localization.md)：先在项目引用处
派生；确需拆散则展示范围与 diff 后转换为项目内 Note/实例/路由。依赖源码始终只读，
组合可拆不代表黑盒 DSP 可拆；保留仍需的插件和素材依赖。

外部 TS factory 可以返回 Config/NoteSource/ArrangementSource 或普通声明式数据。
宿主在其返回边界使用 config.withParameters/withState 或 source.edit 建立局部变体，
不要求提供源码、source map 或“逆函数”，不修改 node_modules。
同一个 factory 用在多个实例时，单实例编辑作用于该引用；定义范围编辑才共享传播。

预设也是配置值，应用预设生成可见 Config 或 preset import + 局部派生，资源引用一起保存。
复制实例复制 Config 与明确选择的 automation，运行 DSP 内存不复制；两实例有独立 runtime
handle。升级/替换创建候选配置，默认新插件参数；只有用户选定的兼容映射才迁移旧参数。
插件内部的私有 arp 或随机声部若未暴露为 NoteSource，宿主不能把它当成钢琴窗可逐音编辑数据。

## 6. GPUI 插件管理器

主工作区提供 Plugins 入口；音源选择器与 insert 添加器复用同一目录服务。
默认只显示可搜索的插件名称/类型列表。详情和工程使用位置按需展开，实例配置是独立编辑窗口；
浏览不创建 DSP 或开启音频设备，不把参数参考表、技术元数据和统计卡片堆在管理器首页。

| 区域 | 内容 / 行为 |
| --- | --- |
| 分类 | 直接显示 All/Instruments/Effects 筛选 |
| 搜索与列表 | 搜索匹配 displayName/vendor/package/pluginId；行仅显示名称、类型及异常，同名时补充 vendor/version |
| Details | 按需显示准确版本、包来源、许可与验证信息，ABI/hash/路径等仅在此披露；不显示参数参考表 |
| Used in project | 按需列出 Channel/Bus/Master 和具体槽位，可打开实例面板或配置；不常驻引用计数 |
| 实例配置 | 独立窗口保留准确实例目标、参数编辑、作用范围、Mix/bypass 和结构操作；浏览目录不重新定位正在编辑的实例 |
| 管理任务 | 安装、验证、更新、重新定位、卸载的进度/失败/取消与重试 |
| 问题恢复 | 缺库/包、错架构、hash/签名/ABI/schema/prepare 错误，不自动替换音源 |

必须区分本机发现、工程声明、已接受实例、运行中占用四个维度。已安装不等于已验证，
目录隐藏不等于禁用工程实例，草稿删除不等于旧图已释放库。显示已知的聚合 faults，
没有 per-instance CPU 遥测就不显示伪造数字。

管理器完整操作包括：

- 从指定 npm 包安装准确版本、接入已安装包、导入本地静态 manifest/平台库。
- 查看所有内置与已发现外部版本，包括尚未被 snapshot 使用的插件。
- 验证候选；添加/替换 Channel 音源，向 Channel/Bus/Master 添加效果器。
- 打开实例、复制、替换、移除、定位引用、修复缺失与重新绑定来源。
- 显式升级/降级并验证参数/state/UI/automation；失败保留旧配置与旧播放图。
- 卸载本工具管理的包版本、移除目录记录；不删除用户原始本地库。
- 查看依赖/许可证和所需平台；同包多插件、同插件多版本均可表达。

缺失插件保留原源码与所有设置。已有 session 保留 last good graph；新 session 没有合法图
时停止，不把自动静音/换内置插件视为成功。可提供显式替换操作，写入用户可见的源码事务。

## 7. 安装、发现、依赖与持久化

插件包增加静态 `oxitone.plugins` metadata 入口，指向按 schema 验证的安装描述文件。
使用项目 pnpm 依赖与 lockfile；管理任务是用户明确的 Install/Update/Uninstall 操作。
刷新列表、打开工程、第一次播放都不隐式下载二进制，不执行全部 npm 包的任意入口来扫描。
本地目录来源必须用户显式加入；不遍历整个磁盘或把任意 dylib 当 Oxitone 插件加载。

安装按准确版本创建依赖任务，展示 package/版本，记录 package.json/lockfile/preimage。
成功后才把新 definition 引入 TS 候选并验证。失败时保持旧工程；外部 lifecycle script
的任意副作用不能靠 lockfile 回滚消除，任务错误和残留需准确报告。
版本并存用 package alias 或版本隔离的正常 module resolution；不全局把所有旧引用重定向
到最新版。平台 hash 根据目标架构解析，不能把 arm64 的 hash 用在 x64 库上。

工程注册改为从 Config 引用自动收集并解析依赖；显式 preload 可用于未使用定义，
不会因此重编译当前音乐。正常 TS 导出携带 definition imports，不需要用户先在隐藏目录
“启用”插件才可运行。CLI/GPUI/native 共用 resolver，库路径、hash、策略在控制侧统一校验。

只读目录缓存保存静态 metadata 与验证 fingerprint，可丢弃；包锁、插件安装描述、
配置 assets 是可移植工程依赖。保存时 TS 已包含配置与依赖入口，不把唯一注册信息存在
ProjectSnapshot 之外的临时 engine registry。项目导出清楚列明需安装的包/平台依赖。

## 8. 验证、信任与版本发布

所有入口显式使用工程插件策略，不再依据 debug/release 隐式切换默认。
默认 signed-only；允许本地 unsigned 的设置是明确工程开发策略，管理器不自动全局放宽。
新版可按 definition 的准确 hash 声明开发例外，仍在加载前检查，不把它冒充代码安全沙箱。
不自动重签名用户库、清除 quarantine 或忽略 hash/签名错误。

验证 helper 有进程级超时、输出预算、崩溃报告，只执行被选择候选的 descriptor/config
验证；它不阻塞 GPUI 或播放线程。通过验证再在正常 engine 中加载并独立校验实际文件，
不信任远端/缓存描述替代本机权威 descriptor。
库及其依赖在使用期保持不可变；同 pluginId/version 不同 hash 是冲突，用新构建版本/准确
来源处理，不覆盖加载中的 Mach-O。旧库在最后实例释放后再由管理任务考虑清理。
卸载要考虑其他 session/process 占用；不能仅凭当前工程引用为零断言可删除共享文件。

format capability 预留 Oxitone ABI、Wasm module、VST3、AU；首个设计实现 Oxitone ABI 2
与现有平台宿主。后续格式通过 adapter 转成同一 Definition/Config/Instance/Target/State
模型，仍参与 TS 保存与管理器；不能把格式字段存在解释为该 adapter 已实现。
