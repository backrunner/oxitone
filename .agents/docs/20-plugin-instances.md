# 插件实例与参数目标迁移

GPUI 的实例添加／替换／删除、外部注册回写与窗口按实例身份跟踪见 [23](23-daw-controls.md)。

Config 为不可变值，Instance 是 Channel/Bus 内独立对象。新增
`channel.instrumentInstance`、`channel.effectInstances`、`bus.effectInstances`，`addEffect`
返回新实例；通过实例对象 `param(id).set(value)/automate(source)` 操作。宿主 insert 参数用
`instance.host.param('mix'|'bypass')`，Channel 宿主仍独立，插件参数中的点号原样传递。

实例执行引用只在 snapshot 生成，TS 保存对象引用，不打印 instanceId。Engine snapshot
1.1 新增 ref.instanceId 与 target.scope（plugin/effectHost），旧 scope 缺省路径暂用于既有
源码；新版 minor 防止旧引擎忽略新增语义。ABI 本边界仍为 1，不将其标记成 ABI 2。
这不是整个 EngineProject/AuthoringDocument 2 的最终迁移。

reorderEffects 接受已有实例对象的排列，保留配置与 typed automation。删除/替换有绑定的
实例先拒绝，须在同一创作事务显式处理相关 lanes；旧 insert-index 路径阻止结构重排，
不自动猜测绑定归属。保留旧 index 所指实例的链尾追加允许；改变旧 target 所指对象拒绝。
参数设置只修改配置，不做实时 setter 或隐式 automation 录制。
原生控制侧解析 instanceId/scope 并生成既有预分配参数目标，实时路径不查字符串表。

完成门禁包括：两个相同插件独立；宿主/plugin 同名参数；重排后同一目标保持；删除/替换
不转绑；跨 owner 错用拒绝；TS Save/新进程重开；Rust/offline 实际参数效果；新 schema、
旧 minor 拒绝、版本/duplicate/dangling validation 与控制侧成本。这些实例/排序边界已通过
TS、native/Wasm offline parity 和真实 GPUI npm C 插件 smoke；记录见
[实施报告](../reports/2026-09-08-project-daw.md)。GPUI 添加/删除/替换、实时 typed setters、
ABI 2 与大工程性能仍未交付，不以排序 smoke 代表全部插件生命周期完成。

旧 1.0 snapshot 仍可解码/原样编码；恢复为可编辑 Project 时明确升级到 1.1，自动分配
缺省实例身份，保留所有音乐数据、旧 target、revision 与数值精度。带新实例或 scope 的
payload 若标记 1.0 则拒绝；1.1 snapshot 恢复保持实例身份。Preset/PluginConfig 不含身份。
复制 ref 为新 owner/addEffect 配置会分配新实例，不复制或复活旧实例身份；仅快照恢复
入口保留外部 wire identity。拒绝的变更不消耗实例序号，不改变后续音乐 entity 分配。

## 效果器源码重排

`orderEffects(owner, [1, 0])` 将调用时的完整实例列表重新排列并返回同一 Channel/Bus。
索引只描述这个表达式的局部排列；绑定始终持有插件实例。排列必须覆盖每个现有实例一次。
Document `effectOrder` 带源 boundary、owner 和当前实例 handles，生成上述局部派生表达式，
复用/新增正常 import；不打印 handles。在 owner 变量所在模块末尾或 block 的 terminal return
之前插入排序语句，先保留既有绑定构造，再改变顺序。不把排序提前到构造表达式，避免
后续 `effectInstances[0]` 改变绑定对象。重复编辑合并末尾同目标排序语句；回到原始顺序
删除该语句。任意多出口/跨模块后续绑定无法隔离时拒绝，不做跨作用域猜测。
工厂继续执行一次，不拆散配置/效果器库；npm 不改动。候选校验保留每个实例、所有绑定、
其他 owner、资源与路由，只允许目标 chain 顺序变化。不可隔离或被后续代码覆盖时拒绝。
GPUI Plugins 上移/下移发送同一事务，Undo/Save/外部代码同步共用文档历史。

求值子进程通过私有结果通道传递稳定 authoring error code；stdout/stderr 不参与错误码解析。
