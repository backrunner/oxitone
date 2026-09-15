# Source authoring：第一批实现契约

本文件记录实际新增的接口。完整目标仍以 [统一设计](../designs/source-daw/README.md) 为准。

## Pattern 规则与输出

`chord(root, quality, options)` 保留生成节点。`arp(input, order, rate, options)` 同时接受
Pattern 和 pitch/Note 数组；数组仍只读取 pitch，Pattern 输入保留共享引用。
六个 chord quality、四个 arp order、力度曲线和 pcg32-v1 洗牌的既有音乐含义保持。
seed 为 safe unsigned number 或 u64 bigint，序列化为十进制字符串；默认 0。

`pattern.outputs` 提供只读 `{note, select, origin}`。chord degree 是原始和弦成员的
1-based 序号 1/2/3（不是调式音级数字 1/3/5）；voice 是倒位/voicing 后的零基声部。
arp step 是零基生成步，不是按修改后 start 排序的数组下标。origin 还保留输入出现位置、
cycleStep/octave；repeat 在原 origin 上增加原始 iteration。它们都不包含源码位置或实体 ID。

```ts
const phrase = arp(chord(60, "major"), "upDown", 0.25);
const variant = phrase.edit([
  { select: { step: 1 }, remove: true },
  { select: { step: 3 }, set: { pitch: 65 } },
]);
```

每批 edit 先在同一 base 上解析全部 selector/expect，再原子应用。删除留空拍，不改变
其他音符的 start/duration/velocity 和 Pattern 长度；插入不重算原有力度曲线。
`set` 为绝对值，`shift` 为相对 pitch/start/duration/velocity。结果逐音符重新校验。
重复 remove 幂等；同目标 remove 和 update 冲突；多个 set 同字段最后值胜出。
expect 是可选的音乐内容约束，失败不猜测目标；GUI 并发 revision 不在此接口内。
已知 selector 跟随上游 root/rate 变化；原坐标不存在时报错。
连续同形 selector 的纯 set edit 合并为一个 edit 节点；degree/voice 等可能互为别名的
不同选择器不跨层归约，保持覆盖顺序。带 expect/shift/结构编辑的层暂保留，不能声称
已完成所有事务归约。`edit([])` 返回原对象。

`edit(operations, { lengthBeats? })` 可在同一原子编辑中显式设置有限正长度；省略时保持
输入长度。edit source node 保存同名可选字段，序列化、恢复与纯 set 归约保留它。
只改变输出容器长度，不重算上游 chord/arp/repeat 的规则和周期。后续对结果调用 repeat
采用新长度；删除音符不会自动收缩。非法长度为 InvalidProject。

literal 用 `{at:{start,pitch,voice?},occurrence?}`；同条件多音未明确 occurrence 时拒绝。
新增音用本编辑谱系中的零基 `{inserted}`，编号是集合操作产生的坐标，不是用户实体 ID。
修改或移动不改变旧 selector 和 origin；通过 `outputs` 取得该输出原选择条件。

## 组合

- `concat(...patterns)`：累计长度并平移音符，selector 加 `{segment,note}`；origin 保留。
- `repeat(count)`：保持原长度周期，selector 加 `{iteration,note}`；来源增加原轮次。
- `slice(start,end)`：`0 ≤ start < end ≤ length`，跨界音符裁剪到有限窗口，并把窗口起点
  映射为 0；保留 selector/origin。这是 authoring 音符窗口，尚不是保持 DSP 发声相位的 clip split。
- `transpose(semitones)` / `velocity(factor)`：前者整数、音高钳到 0..127；后者 0..2、结果
  力度钳到 0..1。均保留来源，不改变长度。

共享 Pattern 永不变更。派生对象仍可由现有 Track builder 放置，`toSpec()` 降级为扁平
引擎音符；旧 scheduler/MIDI 的随机寻址尚未迁移，不能据此承诺未编辑音符的播放概率不变。

## 独立 authoring 格式

`toSource(): PatternSourceDocument` 和 `Pattern.fromSource(unknown)` 保留生成、编辑与共享 DAG。
协议包 zod 类型生成 `schemas/pattern-source.schema.json`，格式版本为数字 1；它不是完整
AuthoringDocument，也不发给 Rust。引擎 wire 仍为原版本，绝不借新名称绕过版本校验。

节点按拓扑顺序保存，子节点必须先于父节点，root 为最后一项，禁止不可达节点。
节点引用是文件内的整数索引，只用于重建共享关系，不是源码 ID 或随机输入。
不保存 entity/note ID、闭包、AST 和机器路径；可选 Pattern name 是显示元数据。
source 内 beat 是有限 JS authoring number，进入 `toSpec()` 才转换 rational BeatWire。
返回的文档为独立副本；加载会重建、验证全部结果，不能信任另存的 flat notes。

预算为 4096 个唯一节点、64 层、单节点 100k 输出、序列化/加载总计 800k 输出；超限报
`BudgetExceeded`。拒绝未知格式、未知字段、循环/前向引用和不合法音乐值。
选择错误码：`EditTargetMissing`、`EditTargetAmbiguous`、`EditScopeConflict`、`SourceChanged`；
其余值/结构错误为 `InvalidProject`，格式不支持为 `ProtocolVersionUnsupported`。

## TS 表达式 writer

`@oxitone/cli/source` 新增 `anchorPatternExpression(fileName,text,start,end)` 和
`writePatternEdit({fileName,text,anchor,source,operations,lengthBeats?})`。后者返回内存候选文本、新锚点和
预期 PatternSourceDocument；不在模块加载时运行 CLI main，不执行用户表达式或写磁盘。
TypeScript AST 校验完整表达式边界；source hash/原表达式不匹配报 `SourceChanged`，
语法错误为 `DraftInvalid`，不可表示的选区为 `EditNotRepresentable`。

输出为普通 `(expression).edit([...])`，不注入 imports、实体 ID 或注释标记。只替换目标
表达式区间，保留外围字节、原换行和上下文缩进。可识别的 literal edit list 在验证规则
一致后局部更新；带注释或需要执行的列表保留原文，在外层追加；后续可归约的纯 set
更新外层列表，不无限包装。无操作保持完整文本字节不变。
发射片段经 prettier 规范化：优先采用文件可解析到的项目 prettier/editorconfig 配置，
无配置时沿用文件自身的引号与缩进习惯，其余按 prettier 默认；模板字面量内的换行属于
字符串数据，逐字保留。Document Service 在评估候选前对改动文件做项目自带
`eslint --fix`（工作区 eslint 兜底，无 eslint 或无配置则跳过）；原文存在既有可修复
问题或 eslint 无法解析该文件时跳过修复，保证修复只落在发射区间内。
显式长度使用普通 `.edit([...], { lengthBeats })`，可验证的 literal options 与 operations
一起归约，计算值与注释保持。无操作且无长度选项时保持完整文本字节不变。

`source` 必须由未来 Document Service 提供该表达式的 accepted evaluation；目前 writer
不自行证明 AST span 与 runtime object 的对应。它是确定性补丁原语，不是已完成的文档
服务：调用方必须重新执行完整候选并校验作用范围、源图和 Rust prepare，成功才发布/保存。
测试已在跨文件共享 Pattern 的引用上写回，保存 TS、删 bundle、重新打包执行，验证音乐与
origin、共享隔离及定义文件字节一致；测试自行写文件，不代表生产 save journal 已实现。

## 实现边界

补充目标见 [MVVM、模块与依赖本地化](../designs/source-daw/07-mvvm-and-localization.md)：
实时缓冲区同步、import/export 改写、npm 结果拆散提示与本地保存均需继续实现。
源码组件见 [Pattern 所有权/import/拆散原语](16-pattern-document.md)。纯文本
writer 本身仍不负责磁盘发布，调用方必须通过文档所有权和候选验证，不可直接保存任意路径。

后续完整工程求值、来源插桩、Document Service 多文件事务、GPUI 音符手势和插件目录见
[18](18-project-daw.md)。Arrangement、automation range、新 Rust random/typed targets、ABI 2、
全部 GPUI 编辑与插件管理任务仍按设计继续实施。`Project.save` 仍保存旧 JSON snapshot，不能当作
DAW Save。删除 JSON 源图再只依赖旧 snapshot 不能恢复生成规则。

可执行示例为 `examples/offline/src/source-edit.ts`。纯 authoring 基准命令：
`pnpm --filter @oxitone/core build && node packages/core/bench/source-authoring.mjs`，测 1k/100k
输出的单音、128 音批量编辑与 JSON 重建；3 次预热、20 次测量，设备/DSP 指标记 null。
