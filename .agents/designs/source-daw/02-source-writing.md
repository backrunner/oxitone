# 源码定位、写回与普通 TypeScript

模块引用与源码所有权遵循 [07：MVVM 与本地化](07-mvvm-and-localization.md)：imports/
exports 与语义编辑进入同一事务；npm 定义只读，需要拆散时先提示并保存到当前项目。

## 1. 双向系统必须满足的性质

记源码文档为 S，依赖与素材版本为 A，执行结果为 Eval(S,A)，用户语义编辑为 E。
源码写回器 W 的基本验收是：

```text
Eval(W(S,E), A) ≡ Apply(E, Eval(S,A))
```

等价关系包括音乐时序、生成/引用范围、随机来源、参数、资源、显式顺序和窗口裁剪，
忽略会话 handle、diagnostic location、build revision；不忽略影响声音的旧 ID 派生随机性，
因为该随机模型将在 04 中移除。比较当前执行并不能证明任意 TS 在所有未来输入下等价，
未来行为由写入的明确生成规则/例外语义决定。

还必须满足：无操作保存源码字节不变；未编辑文件/注释保持；保存再执行不需要缓存；
连续拖动只更新已有 edit，而不无限追加包装；同源码、依赖、素材、seed 的重开结果确定。
最终源码输出由确定性的改写器完成，不把 LLM 重写任意函数作为 Save 的必要环节。

## 2. 解析与来源采集

Document Service 维护 TS Program、模块依赖图、符号/引用、原始文本与每文件 hash。
解析本地工程源码及必要类型声明，不为寻找来源而执行 npm 依赖；包内部不要求有 TS 文件。
sourcemap 只用于构建诊断与位置映射，不承载实体或参数的双向绑定契约。

在原始 TS 擦除前给受控构造/引用/配置边界插桩；只修改临时构建产物，不把 token 写回源码。
SDK 执行产生表达式节点与派生链，结合 AST callsite、调用实例与属性赋值形成来源索引。
需要保持 this、求值次数、参数次序、getter、副作用、异常与 async 上下文，不全局 monkey-patch
Array 方法。无法安全插桩的表达式降级到更外层的返回值/绑定边界。

源索引记录：

- 定义节点、调用节点、引用节点与当前文件版本；同一个 factory 的各次调用分开。
- 节点输出及 typed 参数与原输入表达式的已知映射；共享引用与最后覆盖来源。
- chord 音级、arp branch/octave/step、repeat iteration、slice/region 等生成坐标。
- 当前可采用的 writer：literal、options、output edit、reference replacement、materialize。
- 编辑前的音乐约束与受影响引用集合；带诊断理由的不可改写边界。

不记录音频 PCM 或插件 DSP 指针；不从 meter/实时有效参数推断源字面量。
索引只跟踪执行到的动态对象，不假装覆盖未执行分支、运行时未知 import 或外部 I/O 数据。
有界依赖解析、执行超时、结果大小与追踪预算单独配置；超限保留最后合法图并显示原因。

## 3. 三种身份与版本变化

| 身份 | 产生方式 | 用途 |
| --- | --- | --- |
| session handle | 每执行自动分配，版本内唯一 | 图形选择、事务、实例与图之间引用 |
| source anchor | 原文本范围 + AST/符号关系 + 当前版本 | 精确写回，不写进用户代码 |
| musical origin | 生成器语义坐标与随机 domain | 保持生成/编辑/循环语义，见 04 |

任意两类不能互相替代。相同行号/相同 pitch/相同对象内容，不证明是同一调用或音符。
外部重构后，先由文本变更映射锚点，再核对符号、上下文与音乐前置条件；只有唯一匹配
才能重定位旧事务。重复候选则报告冲突，已保存 TS 照常按它的新语义执行。

全新进程可以重建所有音乐与生成坐标，不必复原旧 GUI handle。引用恢复只需匹配当前
TS 的对象图；旧窗口选择可丢失，音乐语义不能依赖缓存里的匹配结果。
格式化与符号重命名可能不改音乐，但必须更新 source anchors 和 source revision。

## 4. 写回策略是确定的优先级

1. 明确的规则编辑：改对应构造调用参数；共享变量只有在用户选择定义范围时才修改。
2. 唯一 source literal：改原字面量/属性；有后续写入时定位最后生效写入，不改已被覆盖的值。
3. 输出/实例编辑：在最近的已知输出边界建立 `.edit` 或 `.withParameters` 等正常表达式。
4. 共享实例编辑：在选中 placement/instance 的引用位置派生变体，其他引用保持原定义。
5. 任意数据生成：在 `Pattern.generate` 或可识别返回边界外包装 edit，不尝试求解 callback。
6. 用户明确 Materialize：只重写选定范围，保留仍被其他对象引用的上游定义与 imports。

例：`track.place(makeSection(...))` 的结果可用 ArrangementSource 包装；对 shared
makeSection 本体不做跨调用修改。若它直接对 Project 产生副作用且没有可隔离结果，不能
删掉调用后仅回填最终快照。提供 `project.capture(() => buildSection(project))` 的显式
受控边界迁移：只允许 SDK declarative 写入，创建操作先写 transaction-local builder，
内部读取看见同一局部视图，成功后整体发布；访问外部 mutable 对象、返回逃逸引用、
调用 play/compile/外部 I/O 的 helper 不标为可安全 capture，须改为返回值接口。
该边界不能隔离任意 JS 副作用，不以它作为通用程序事务。

新创建音符/Clip/插件引用在最近的可写集合中插入；若当前代码只有不可扩展表达式，
可以生成命名局部常量并引用它，或者写正常 import 的 `*.arrangement.ts` 模块。
新模块不是隐藏覆盖数据库，也不使用 generated bindings 目录。

## 5. 输出可读性与编辑归约

重用现有别名、引号与缩进，修改文本区间而非整文件重新打印。插入新 import 时按 TypeScript
模块语义处理命名冲突与 type/value imports；不执行自动删除可能有副作用的 imports。
保留注释与纯粹格式差异；对语法损坏的局部暂存编辑，不能用旧 AST span 覆盖新文本。

已有 `.edit([...])` 由 writer 识别，不需要隐藏 marker。同 base 的 edits 归约为规范列表：
每个原对象最多一个最终 patch，删除吸收属性修改；插入后再删除可消除该插入。
不得为了归约而重编号 base 音乐坐标、改变 creation-side effects 或随机 domain。
嵌套 edit 只有证明选择范围和 origin 等价才合并；否则保留有意义的嵌套。
注释位于被删除区域时迁移到相邻相关语句或在 diff 显示，不默默丢弃大段用户说明。

完整 source span 不是可长期存储的 selector。保存后的音乐 selector 是可读的生成坐标或
条件，仅对需要内容消歧的输出带 expect；正常 root/rate 变化可继续应用已保存的 step/degree
规则。约束失败时以普通 SDK 错误报告，在 CLI 和 GPUI 中一致。
不能在 GUI 自动挑一个新目标，却让同一 TS 的 CLI 执行得到另一个结果。

## 6. 可编辑能力与普通 TS 的边界

所有合法工程继续可执行与预览。编辑能力按目标、属性与作用范围判定，不整份文件一刀切。
Node 可运行一般 TS，程序所有者仍需控制网络、时间与 Math.random 等不稳定输入。
Document Service 记录显式文件/包/素材版本；提供 seeded random 和受控 asset/data import，
给需要可重放保存的生成器使用，不能声称自动捕获了任意系统调用。

对 nondeterministic factory：可编辑稳定配置并显示动态结果，但不能以两次不同输出证明
局部写回等价；需要用户明确固定输入/seed 或 materialize 选定的有限结果。
某个插件 DSP 内部行为是否确定由 plugin capabilities 声明，区别于 authoring 的可重放性。

编译产物 `.mjs` 可执行。要编辑源码，需可访问的 TS source bundle 和 dependency mapping；
只有 sourcemap/sourcesContent 而没有原文件时可导出一个新的源码工程，不能假称修改了原工程。
没有原源码的 snapshot 工程可导入生成声明式 TS，声明它恢复了音乐数据而非原始函数和注释。

## 7. 依赖与资产

正常工程由 entry TS、模块、package.json/lockfile、插件描述与显式资产构成。
asset base 按源模块 URL 解析；source rewrite 不能改变 import.meta.url 的资源基准。
大 sample、IR、opaque plugin preset bytes 保存为内容寻址资产，TS 保存其引用/格式/hash；
它们与 WAV 一样是显式工程依赖，不是隐藏音乐状态。

预设应用的结果回写配置或正常的 preset import + 局部派生；依赖文件若可变，纳入版本核对。
目录缓存只加速解析与验证；恢复日志可保存未提交源码，但清空后只丢未保存工作与历史，
不能改变最后一次成功保存的工程。包锁和显式资产不得称为可丢弃缓存。
