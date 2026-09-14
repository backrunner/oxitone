# 文档事务、IPC、GPUI 与保存

MVVM 投影、代码缓冲区桥接与防回环规则见 [07](07-mvvm-and-localization.md)。实时同步
不等待 Save；仅文件 watch 无法同步外部编辑器未保存文本，必须明确模式与桥接状态。

## 1. 单一文档服务与多个派生版本

Document Service 在 Node 控制进程运行，拥有文本、源码草稿、undo 历史与事务。
代码编辑器可以是外部 VS Code，也可以是 GPUI 的源码查看/编辑面板；首版必须支持外部
文件 watch 与打开到源码位置，完整内置代码 IDE 不阻塞 DAW 编辑功能。

版本分开：`diskRevision`（落盘）、`sourceRevision`（服务内草稿）、`evaluationRevision`、
`graphRevision`（已准备/已激活）和 `catalogRevision`（插件目录）。每次文档构建记录
精确输入文件/依赖/素材 hash；所有版本在同一个 session 内单调递增。
同一文本草稿可构建失败，GUI 不用“最新版本”一个标签掩盖当前播放仍是旧图。

| UI 状态       | 含义                                                       |
| ------------- | ---------------------------------------------------------- |
| Saved         | 该源码集合已持久发布；播放图另显示对应版本                 |
| Modified      | 有有效但未保存的源码事务，可试听、Undo、Save               |
| Building      | 正在构建候选，显示轻量拖动投影与上次接受的工程             |
| Invalid draft | 草稿有错误；保留草稿与最后合法播放图，不把旧声音标成新结果 |
| Conflict      | 外部修改与未提交事务冲突，提供差异与解决入口               |

## 2. 一次图形编辑的完整生命周期

1. pointer-down/gesture-begin 捕获 source/graph revision、选中范围与前置值。
2. GPUI 即时呈现受限编辑投影；pointer-move 只更新同一事务最新值，保持 60 Hz 交互。
3. 参数试听通过 typed target 发预览值；结构编辑节流构建候选，不能每个像素执行 TS。
4. gesture-end 形成一条可撤销语义命令，writer 生成内存源码补丁。
5. 从完整草稿求值，验证实际变化与预期 Apply(E) 一致；Rust 校验/prepare 候选。
6. 候选成功后发布文档接受结果与图切换请求，明确区分 prepared 和 audible/active ack。
7. Save 发布当前有效源码版本；无须每次点击 Save 另询问用户批准 diff。

用户可以连续操作未保存的草稿；后续事务基于上次服务接受版本，不基于最后磁盘版本。
旧 candidate 完成时若已有更高 source revision，只能缓存或丢弃，不能覆盖新文档。
试听失败/取消回到当前已提交 authoring 参数及 automation 优先级，不能恢复成手势开始时
一个已经过时的裸值。pending note edits 可显示，但不宣称已被音频引擎采用。

## 3. 统一的 typed edit command

以下为内部协议形状，handle 均由服务生成，不要求 TS 用户手写：

```ts
type ParameterTarget =
  | { kind: "channel"; owner: Handle; parameterId: string }
  | { kind: "instrument"; instance: Handle; parameterId: string }
  | { kind: "effect"; instance: Handle; parameterId: string }
  | { kind: "effectHost"; instance: Handle; parameterId: "mix" | "bypass" }
  | { kind: "bus"; owner: Handle; parameterId: string }
  | { kind: "send"; send: Handle; parameterId: "ratio" }
  | { kind: "sampleClip"; owner: Handle; parameterId: string }
  | { kind: "project"; parameterId: "tempo" };

interface EditRequest {
  protocolVersion: "2.0";
  sessionId: string;
  requestId: string;
  transactionId: string;
  baseSourceRevision: string;
  phase: "begin" | "update" | "commit" | "cancel";
  operation: SemanticOperation;
}
```

operation 用各自 schema 定义 note/placement edit、source parameter、automation range、
plugin config、instance insert/remove/replace/reorder、routing、sample edit 与材料化。
所有参数 ID 原样传递；label、字符串拼接路径和数组显示下标不能用作唯一地址。
Rust 接收的已编译 command 再将 handle 解析为定长节点/参数索引，回调不解析字符串或 JSON。

IPC 区分 request、response、event，响应必须关联 requestId；状态事件不释放未应答请求。
相同 requestId 幂等返回同结果。结构事务按顺序提交，不可按类型只保留最后一条。
gesture update、meter、目录查询可合并/丢过时数据；commit/cancel/save/transport ack 不丢。
schema 定义 max frame、队列容量、超时与取消行为，queue full 返回可重试错误，不报假成功。
本地 Unix socket 沿用受限权限；重连先握手 session/version/hash，不重放旧 session 的 handle。

## 4. 外部代码修改与 Undo

文件 watch 是另一种 EditRequest 来源。无本地草稿时接受新磁盘版本并重新构建。
有草稿时，对每个被改文件用 baseline/ours/theirs 做文本三方合并，再重做 AST、语义前置条件
与结果差异核对。文本能合并并不意味着生成器实例仍对应同一对象。

可唯一重定位的未提交操作在新 baseline 上重放；删除目标、同名复制、规则改变触发 expect
失败、同字段竞争则保留两份文本及操作列表，显示冲突。不能先保存再让用户发现改错实例。
注册/包/素材变化参与依赖版本校验。仅布局/注释修改更新对应版本，不无谓重编译 DSP。

Undo 栈属于 Document Service，图形/内部代码操作共用。一条拖动、一组多选移动、一个
插件替换都是一个事务；撤销整组后重新构建正确源码结果。外部编辑形成 baseline 事件，
不偷偷纳入可覆盖外部文件的旧整文件回滚；跨外部编辑撤销需要安全重放逆 patch，失败则冲突。
连续同参数手势可按明确时间/焦点范围合并；跨实例、代码编辑、保存点不自动合并。

## 5. 保存协议与崩溃恢复

正常 DAW Save 的对象是服务中通过 authoring/Rust 验证的源码集合；代码编辑器仍可保存
暂时无效 TS，GUI 标记 Invalid draft/last good。两种情况不要共享一个误导的“可播放已保存”。
用户可另选保存无效草稿，但必须明确它不代表新图已接受；不能为绕过错误自动删插件或音符。

保存步骤：

1. 固定 source revision，收集 read set、write set、每文件 baseline hash 与候选内容。
2. 将新资产写入不可变内容寻址位置并 fsync；旧资产不删除。
3. 写恢复 journal：事务 ID、文件列表、preimage/postimage hash、备份/临时路径、阶段。
4. 获取工程内协作写锁，复核依赖/read set；发现变化先重建/合并，不继续覆盖。
5. 同目录临时文件写入并 fsync，逐项 rename、fsync 目录；记录每个发布阶段。
6. 复读 postimage，完成 journal 并发布 Saved(diskRevision)，再通知 watcher/CLI 更新。

多个 rename 不是跨文件原子操作。GPUI/CLI 的 Document loader 在有未完成 journal 时先恢复，
采用完整旧代或完成新代；不能把半套源码执行成新工程。原生 `node` 直接 import 无法被保证
永远避开跨文件中间态，跨文件保存期间需使用统一 loader 或等待事务完成。
非协作外部编辑器仍可能在核对与 rename 间竞争；没有跨应用原子 CAS 的承诺。保存前备份、
watch 观察、发布后复核与恢复副本用于发现/恢复竞争，不宣称绝对消除所有覆盖窗口。

恢复时若某文件已被外部改成第三种 hash，不自动覆盖；保留 journal/备份并进入恢复冲突。
用户确认删除的源码与正常编辑一样在事务内处理，不删除其他目录文件。
包安装操作含 package.json/lockfile/node_modules 变更，使用独立依赖任务与恢复日志，
安装完成后再提交使用新版本的源码；Cancel 不保证任意 package lifecycle script 可逆。

## 6. GPUI 操作范围与编辑反馈

- Playlist：插入/移动/复制/删除/裁剪/分割/替换 Clip，选择规则、实例或轮次；默认不 ripple。
- Piano roll：多选、插音、拖动、时长、力度、删除、量化/移调；定义视图修改共享 Pattern，
  实例视图派生变体。显示生成来源与仍共享的引用，常规手势不反复弹 scope 确认。
- Automation：参数面板改规则，曲线画笔改明确时间域；显示 source 与 lane 的范围区别。
- Mixer：音源/insert/sends、host 与 plugin 参数、路由、mute/solo；替换和重排维护绑定。
- Samples：非破坏性素材编辑、Clip 窗口、切片和键/力度分区；素材解析异步控制侧完成。
- Plugins：目录、依赖、管理任务与实例面板共享文档 session，详见 05。
- Code：定位到生成表达式/实际写回处，查看当前事务 diff 与来源链，可用外部编辑器继续写。

播放/定位命令不写工程源码；缩放/主题/选择等偏好不进入音乐 graph。
autosave 若启用，复用相同事务验证与发布规则；不能在草稿冲突/构建失败时暗中覆盖磁盘。
所有视图以服务接受的结果为准，轻量拖动投影有明确 pending 标识与错误回退。
