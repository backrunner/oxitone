# 现有生成与组合 API 的图形编辑审计

目标方案以 [完整代码 / DAW 架构](../designs/source-daw/README.md) 为准。以下逐项源码事实
保留为迁移审计；实现限制与备选写回建议已由新 source graph/typed edits 设计统一替代。

状态：讨论草案，2026-09-08；补充 [源码 DAW 方案](2026-09-08-source-backed-daw.md)。
本文盘点当前源码实现，不代表下列写回器已实现。沿用无源码显式 ID、无隐藏绑定标记、
保存结果可独立执行的约束。这里把音乐领域的生成器、组合器和派生 builder 一起审查，
并非说每个函数在编程语言定义上都是高阶函数。

## 1. 范围与公开入口核对

基于 `packages/core/src/index.ts`、`packages/sdk/src/index.ts`、`packages/web/src/index.ts`
及其实现逐项核对；后两者复用 core authoring，没有另一套音符生成器。

| 类别                   | 当前提供                                                                                                   | 实现位置，相对仓库根目录                                             |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| 音符生成               | `chord`、`arp`                                                                                             | `packages/core/src/chord.ts`、`arp.ts`                               |
| 自动化生成与组合       | 24 个 namespace 方法，完整列表见第 5 节                                                                    | `packages/core/src/automation/namespace.ts`                          |
| 自动化绑定与时间域     | `addAutomationLane`、Channel/MixerChannel 的 `automate`、lane combine/loop/lastBeat                        | `project.ts`、`automation/lane.ts`、`channel.ts`、`mixer-channel.ts` |
| Pattern 编排与变换     | Track `add/pattern(...).at`，Clip `loop/last/transpose/velocityScale/probability/enabled`、`durationBeats` | `track.ts`、`pattern-clip.ts`                                        |
| Sample 编排与派生长度  | `track.sample(...).at`、`fitBeats/fitBars/fitToContent`，loop、rate、tempoSync                             | `sample.ts`                                                          |
| 采样切片与键盘分区生成 | `slicer`、`multisampler`、`grandPiano`、`softPiano`                                                        | `instruments.ts`、`multisampler.ts`、`piano.ts`                      |
| 音源与效果器声明转换   | `wavetable`、`sampler`、`effect`、`convolver`                                                              | `instruments.ts`、`effects.ts`                                       |
| 预设派生与应用         | `createPluginPreset/createChannelPreset`、`presetInstrument/presetEffect`、`applyPreset`、`applySettings`  | `preset.ts`、`channel.ts`                                            |
| 时钟与共享路由         | Project tempo/signature builders、Track tempo/use、Mixer send/insert 操作                                  | `project-timeline.ts`、`track.ts`、`mixer-channel.ts`                |

当前没有公开的音符 `Euclidean`、`Humanize`、`Quantize`、`Strum` helper；02 的对应条目
是后续候选。`automation.quantize` 已存在，但它量化 0..1 控制值，不量化音符拍位。
也没有公开的 Pattern `repeat/concat/slice/map` 方法；Pattern 的 notes 是只读数组，
工程中的平铺、拼接、过滤主要是普通 TS 数组/循环和用户 helper。
SampleEditSpec 有 trim/level/tone/normalize/fade/crossfade 声明，不能因此虚构同名链式方法。

Project/Pattern/fromSnapshot、项目文件读写、预设读写/校验、Sample import/inspect/cache、
MIDI/WAV 导出、Session/native/web 播放控制、ID/Beat/时钟工具和 plugin UI 注册也已检查入口。
它们分别是构造恢复、I/O、执行或呈现能力，不增加另一类可逆的音乐生成函数。
示例目录的歌曲 helper 属于用户 TS 情形，不能算作 SDK 已发布的编辑适配器。

## 2. `chord`：和弦规则与单音结果

实际支持 major/minor/dim/aug/sus2/sus4 六种三和弦；不是任意七和弦/扩展和弦生成器。
先由 root + intervals 生成音高，再 inversion 旋转低音向上八度，再处理 open/drop-2
并按音高排序；最终高于 127 的音高截到 127。所有音符共享 start/duration/velocity。
`voice` 是结果排列提示，不是始终代表根音/三音/五音的稳定身份。

| 用户操作                      | 默认写回落点                                            | 为什么不能随意反推                                |
| ----------------------------- | ------------------------------------------------------- | ------------------------------------------------- |
| 和弦面板调整 root/quality     | 原调用参数                                              | 这是用户明确编辑规则，会更新相应生成结果          |
| 调整 inversion/voicing        | 原 options                                              | 会重排声部；不能沿用旧音符 index/voice 当音级身份 |
| 整个生成结果统一平移/缩放时值 | 作用范围明确时修改 start/duration；同时核对 lengthBeats | 多个调用共享 options 时要改调用侧，不能影响别处   |
| 单独升降一个音                | chord 输出后的 Note 属性修改                            | 即使碰巧变成另一种和弦，也不自动改 quality        |
| 单独延长/缩短、延后、改力度   | chord 输出后的 Note 属性修改                            | 改统一 duration/start/velocity 会连带其他音符     |
| 删除一个音、补入第四个音      | 输出集合删除/插入，保留其他音符字段与顺序               | 六种三和弦参数无法表达任意二音/四音集合           |
| 把一个音放到另一条 Track      | 在此实例派生两个 Pattern/放置关系                       | 需要决定目标 Channel，属于跨对象结构编辑          |
| 在循环和弦的某一轮改一个音    | 先定位轮次，再派生局部 Pattern                          | 改定义或整个 Clip 会影响其他轮次                  |

需要测试的特殊情况：`chord(124, "aug")` 输出 `[124,127,127]`；两个 127 不可通过
音高相等条件区分。open 与 inversion 的输出必须保留从原音级到最终声部的来源关系。
当前 helper 没有显式 Note ID；新增来源信息只能留在临时执行元数据中。

## 3. `arp`：一个输入音可能对应多个输出事件

实际签名是 `arp(notes, order, rate, options)`；`rate` 是每一步的拍数，不是 Hz。
输入可以是 pitch 或 NoteInput，但实现只读取 NoteInput.pitch；原 start、duration、
velocity、chance、voice、tags 不会被带入结果。不能把输出力度修改写到输入 Note.velocity。

`up/down` 按音高排序；`upDown` 在升序后接不含两端的倒序，`[60,64,67]` 得到
`[60,64,67,64]`；random 用固定 seed 做一次洗牌。octaves 在外层向上逐八度堆叠这个序列，
并把越界音高截到 127。它不是播放每一轮都重新随机的 realtime arp。
start = 输出步号 × rate，duration = rate × gate；velocity = 基础力度 × 全序列线性力度曲线。
`lengthBeats` 只设置 Pattern 长度，不自行把音符填充或重复到该长度。

| 用户操作                                  | 默认写回落点            | 必须保留的结果语义                                  |
| ----------------------------------------- | ----------------------- | --------------------------------------------------- |
| 生成器面板改 order/rate/gate/octaves/seed | 原调用参数/options      | 重新生成全部相应事件，是显式规则修改                |
| 面板改 velocity/velocityCurve             | 原 options              | 同时核对总事件数变化对插值分母的影响                |
| 只改某次出现的音高                        | arp 输出事件            | 不能修改输入 pitch，导致排序改变或多个出现一起改变  |
| 只拖动一个事件                            | 输出 start 的例外       | rate 和其余事件保持原样；不隐式 ripple              |
| 单独拉长一个音                            | 输出 duration 的例外    | gate 仍支配其余音符；核对 Pattern/Clip 尾部截断     |
| 单独改一个力度                            | 输出 velocity 的例外    | 不能调整全序列 ramp 的两个端点                      |
| 删除一个事件                              | 生成后删除该事件        | 留下空拍；后面事件 start 和力度 ramp 结果不重新编号 |
| 插入一个事件                              | 在输出集合插入          | 不往 arp 输入补 pitch 后重新生成整个序列            |
| 调换两个事件的顺序                        | 明确交换它们的时间/内容 | 不以修改 up/down/order 或换 seed 近似实现           |
| 只改某个 octave 或回程事件                | 对该输出分支的事件处理  | 其他 octave 和去程引用保持不变                      |

例如 arp 原结果 `[60,64,67,64]`，用户只把最后的 64 改成 65，目标是 `[60,64,67,65]`。
把输入 `[60,64,67]` 的 64 改为 65，会得到 `[60,65,67,65]`，不是这次编辑。
random seed 改变、重复输入 pitch、octave 截顶都会让旧输出匹配失效，不能靠音高定位。

输出事件来源至少记录：本次调用、输入音的来源、排序/洗牌后的 cycle 位置、去程/回程
分支、octave 和最终事件位置。相关字段是计算证据，不要求用户把它们写成身份标记。
在生成器改变后，“编辑第 n 步”与“编辑某输入音的某次出现”是不同的可读规则；
保存前就明确采取哪一种，不能悄悄在两种含义之间迁移。

## 4. 组合后的编辑应落在最靠近目标的输出边界

以下是现有 API 能表达的输出形式示意，没有新增 helper 或手写 ID：

```ts
const harmony = chord(60, "major");
const generated = arp(harmony.notes, "upDown", 0.25);
const lead = new Pattern({
  lengthBeats: generated.lengthBeats,
  notes: generated.notes.map((note, step) => (step === 3 ? { ...note, pitch: 65 } : note)),
});
```

这是“修改 arp 输出的第 4 步”。harmony 可以继续被其他 Clip 引用。
若只编辑 lead 的某一个放置实例，再在放置层选择派生变体，不替换所有 lead 引用。
不同意继续跟随 arp 重算时，可明确将本次输出转换为普通 Note 数组；只展开该边界。
上面的代码说明表示能力，不证明重建 Pattern 后的概率/创建顺序已无损，仍须主方案第 9 节验证。

对 `chord → arp → JS flatMap → Clip.transpose → loop`，局部操作不能直接跨越整条链
修改 chord。每层记录“改输入会影响哪些输出”，优先选择满足目标差异的最外层局部表达。
存在多个可行反推时，以用户选择的规则/结果/实例/轮次层次为依据，不以代码字符最少为依据。

需要特别区分绝对与相对修改：把音高设为 65，与“始终比生成结果高半音”在今后改 root
时会不同。默认音高/力度设值可保存为绝对值，移调等相对命令保存为运算；拖动时间的绝对
位置与相对偏移也须固定产品语义。撤销和后续编辑都更新同一变体，避免多层 map/Pattern 套娃。

## 5. Automation：完整 24 个入口的局部编辑分类

依据 namespace 的接口与返回实现核对：13 个 source 工厂（含 wave 别名）、6 个一元/范围
组合、5 个二元组合，共 24 个。namespace 返回 AutomationSource 的方法不接受任意 JS 回调。
AutomationSource 仅有 `toSpec()`，没有 `source.map(fn)` 或 TS 实时 evaluator。

| 方法       | 当前含义                                  | 局部画线/改点应如何处理                                    |
| ---------- | ----------------------------------------- | ---------------------------------------------------------- |
| `constant` | 常量                                      | 全局调值改 value；一小段改变用区间覆盖/分段曲线            |
| `curve`    | 带 interpolation/逐点 curve 的控制点      | 改明确点或曲柄；新增点核对左右段与相邻斜率                 |
| `polyline` | linear curve 别名                         | 直接改控制点；不能视为无限精度波形采样                     |
| `line`     | 两端点 linear curve，起点 beat 0          | 调端点可保留 line；增加折点改为 polyline/curve             |
| `gate`     | period/duty/phase/on/off 的周期门         | 改整个门规则改参数；一个脉冲例外在输出时间区间处理         |
| `chance`   | seeded sample-and-hold，可 smooth/restart | 改 probability 是规则；固定一次结果不是改 seed/probability |
| `wave`     | 选择 wave kind 与周期/相位/范围           | 面板改参数保留调用；局部涂画覆盖该区间                     |
| `sine`     | sine wave 别名                            | 同 wave；不能用有限折线宣称精确替代完整正弦                |
| `cos`      | cos wave 别名                             | 同 wave，保留本来的相位含义                                |
| `triangle` | triangle wave 别名                        | 局部编辑可以分段表达，但需核对周期和拐点                   |
| `saw`      | saw wave 别名                             | 区间覆盖必须保留边界跳变与右连续规则                       |
| `ramp`     | ramp wave 别名                            | 同上，不能把周期重置抹平                                   |
| `square`   | pulseWidth 控制的 square wave             | 只改一个脉冲不修改所有周期的 pulseWidth                    |
| `map`      | 把归一化 input 线性映射到 min..max        | 唯一且非零区间才可能反推；min=max 丢失输入信息             |
| `clamp`    | 把输入钳到上下界                          | 饱和输出对应许多输入，默认在结果侧覆盖                     |
| `invert`   | 1-input                                   | 可代数反推，但共享输入/用户编辑层次仍可能要求局部覆盖      |
| `quantize` | 将控制值量化为 steps 个级别               | 多对一；拖动输出不能唯一确定量化前的值                     |
| `scale`    | input × factor                            | factor=0 不可逆；其他情况也要核对范围与后续钳制            |
| `offset`   | input + amount                            | 明确编辑整体偏移才改 amount；局部差异在结果侧处理          |
| `mix`      | 两输入按标量 amount 混合，默认 0.5        | 一个输出对应多组左右值；amount 不是 AutomationSource       |
| `add`      | left + right                              | 不自动选择改左还是右；通常覆盖最后结果                     |
| `multiply` | left × right                              | 多解/零因子；不能强行除回某一支                            |
| `min`      | 两输入取较小值                            | 被遮蔽分支的信息丢失；交点附近活动分支会改变               |
| `max`      | 两输入取较大值                            | 同 min，不能把当前活动分支推断成永久编辑对象               |

curve 支持 step/linear/smooth/exponential/bezier。移动一个控制点本就会影响相邻段；
若用户只画选中区间，需要保留区外段以及进入/离开区间的准确端点，不把邻接变化藏起来。
wave/gate 周期与 lane loop 是不同层；覆写 source 的一段可能会在每个 lane loop 重复。

### 5.1 一部分区间覆盖可用现有组合器表达，但有明确条件

对于无 chance、输入已在 0..1、无需特定单轮 lane 定位的 source，可用 step curve 作为
区间掩码 m，用现有组合表示 `(1-m)*base + m*replacement`。它仍由 Rust 求值；
不能把 JS 条件函数直接传给 automation。实际输出要通过 range、深度 64/节点数 256 校验。
组合器不简化共享树，复杂原表达式或连续多次覆盖很快消耗预算，因此需合并编辑区间，
并考虑专门的版本化区间覆盖表达。后者是候选新能力，不是当前 schema 已有的 kind。

掩码需要处理 beat 0、[start,end) 右连续、原曲线局部时钟、lane loop/count/lastBeat、
replace/add/multiply/max 的合并顺序与参数物理映射。lastBeat 是 hold 语义，不能当作
“从此刻自动撤销覆盖”；给目标另加一条 lane 也不保证只影响某段。
某些参数由自动化覆盖初始值，UI 拖动旋钮要明确修改初始设置还是写入 lane。

### 5.2 `chance` 是包装组合时的实际阻塞项

Rust `crates/transport/src/automation/compile.rs` 把 root `0` 和 child-index path
传入 chance seeding；这个 path 是 automation wire 表达式路径，不是 TS 文件 AST 路径。
把原 chance 包入 multiply/add 会从 `0` 移到 `0.0` 等路径，即便种子不变，也可能使
区间外的随机结果一起改变。不能简单套用上述掩码方案并宣称区间外无变化。

随机来源寻址要独立设计兼容语义；新 source kind、lane 级覆盖或透明包装都要评估其
seed path、restart/loop、MIDI/offline/实时一致性。禁止让隐藏编辑缓存决定最终随机结果。
冻结某一次随机轨迹需要声明时间范围与是否放弃以后循环的变化；不能自动冻结整个 source。
tempo lane 还禁止 chance/restart，编辑后须重新烘焙时钟，不能只改视觉 BPM。

## 6. PatternClip 与 SampleClip 的逐项结论

| 当前操作/字段               | 编辑策略与边界                                                               |
| --------------------------- | ---------------------------------------------------------------------------- |
| Track `add/pattern(...).at` | 定义与放置引用分开；改某次 at 的结果，不一定改输入 bar 变量                  |
| PatternClip `loop`          | 改 count 是规则；改某轮是轮次例外/拆分，不能修改整个 Pattern                 |
| PatternClip `last`          | 绝对 exclusive end；移动起点后保持端点与保持长度是不同操作                   |
| `durationBeats`             | 保留显式长度/截断语义；不能和 loopCount/lastBeat 随意混用                    |
| `transpose`                 | 整个 Clip 改 transpose；单音可在实例变体中反算半音差，越界需拒绝，不静默截顶 |
| `velocityScale`             | 输出钳到 0..1；零比例、饱和、目标超出当前比例可达范围时不能简单反除          |
| `probability`               | 规则作用于各 note 的确定性抽样，乘 note.chance；不是给整个 Clip 抽一次开关   |
| `enabled`                   | 禁用仍保留编排；图形删除不能悄悄等同静音                                     |
| Sample `fitBeats`           | 直接改长度，但维持已有 off/stretch/repitch 行为                              |
| Sample `fitBars`            | 按起点拍号折算；改为固定拍数会失去跟随起点拍号的语义                         |
| Sample `fitToContent`       | 长度来自音乐长度或素材/trim/时钟；手动拉边需显式切换到用户指定长度           |
| Sample loop                 | 单轮编辑需保留原 sample 播放相位与循环边界；独立 Clip 从头播放通常不等价     |
| Sample rate/tempoSync       | 速度、变调、保调伸缩互不等价；不能为了符合画面长度随意替换模式               |
| Sample gain/pan/enabled     | 直接实例参数；注意 gain 与 SampleEditSpec.level 的共享范围不同               |

`.transpose` 越界是 scheduler 校验错误；chord/arp 的生成阶段截顶行为与它不同。
Sample trim/normalize/fade/crossfade/tone 的结果由 Rust prepare 计算；局部改采样实例时
如需变更共享 Sample edits，派生 Sample 引用并复用素材，不能重写原 WAV 或影响所有实例。

## 7. 其他会被“打破”的生成结构

| 入口                                     | 局部编辑与落盘策略                                           | 特殊约束                                                                     |
| ---------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `slicer(..., {slices:{grid}})`           | 改整个 grid 改参数；拖单个切片边界需解析为显式 slices        | Rust 按 prepared frames 等分；要保留精确 frame 与样本编辑坐标                |
| `slicer(..., {slices:{onset}})`          | 改敏感度是规则；手动分割/合并改显式 slices                   | onset 在 Rust prepare 求值，当前 snapshot 没有完整解析结果；需控制侧结果查询 |
| `slicer` 显式 slices                     | 编辑 start/end 与该片 level/pan/rate/reverse                 | 隐式 end 取下一片 start；移动边界会影响邻片，排序还决定 MIDI 触发编号        |
| `multisampler`                           | 直接改 regions 的 sample/rootKey/keyRange/velocityRange/gain | 1..256 区域且键/力度矩形不重叠；region_N 是资源地址，不是跨版本编辑身份      |
| `grandPiano`                             | 声音参数改 options；单个自动分区边界需要展开此音源的 regions | 原算法按 rootKey 排序/中点分键区，按层数均分力度；移动 rootKey 会改变相邻区  |
| `softPiano`                              | 同上，只对其输出分区操作                                     | 原算法只选最弱两层；不要为单区修改去改变整个 bank 或 grandPiano 的其他引用   |
| `wavetable`                              | 回写嵌套 options 的对应字段/枚举                             | 扁平参数不等于原 TS 字段名；modulation/macros 数组展开，删除或调序要跟踪绑定 |
| `sampler`                                | 回写 options 或这次使用的 Sample 引用                        | amp/loop 等有字段降级；修改共享采样只影响此音源时需派生引用                  |
| `effect`（所有 16 种 EffectKind）        | 物理参数改 parameters，宿主 mix/bypass 改 options            | 各 kind schema/单位不同；effect("gate") 与 automation.gate 完全不同          |
| `convolver`                              | 保留 impulse Sample 对象引用表达式，例如 sample.id           | 这里访问 SDK 产生的 id 可行，不要求用户填写 ID 字符串                        |
| `presetInstrument/presetEffect`          | 在应用调用侧覆盖目标字段或替换引用                           | 不默认改共享预设文件；资源仍要正确 remap                                     |
| `applyPreset/applySettings`              | 跟踪最后设置来源，在本次应用之后表达局部修改                 | 有批量副作用、资源导入和校验，不能把整个调用当纯对象构造删除                 |
| `createPluginPreset/createChannelPreset` | 保留捕获位置/时机，编辑其返回数据需按使用范围派生            | 捕获之后的对象修改不会自动改之前的预设值                                     |
| insert/send/Track.use                    | 局部改引用、连接与链顺序                                     | insert 重排必须同时迁移按槽位绑定的 automation；所有共享路由影响都要可见     |
| tempo/signature/Track.tempo              | 回写明示时钟参数；局部拉伸编排不默认修改全局 tempo           | tempo lane 可覆盖 tempoMap；有效时钟不能从单个显示值反推                     |

Slicer 显式片段按 start 排序，触发音高 = triggerNote + 排序后位置。插入一片会使后面的
触发音高换意义；若用户要求原有触发仍指向原音频，需要同步迁移相关音符并检查共享引用。
删除一片之后显式数组会压缩，也没有通用“空 slice 占位”可假设，必须定义删片的音乐语义。
钢琴区域展开保留 Sample 引用和选定 options/defaults，不能把 grand/soft 的默认音色差异丢掉。

## 8. 对总方案的修正：按输出类型提供编辑规则

通用“加条件”只适合部分编排场景。建议给每个已知生成/组合入口提供：参数字段映射、
输入到输出的来源关系、共享影响范围、允许的结果编辑、局部显式化编码和验收前置条件。
这些是确定性的编辑规则，不是要求每个函数都有数学逆函数。

形成三条主要路径：

- 有限集合（Note、placement、slice、region）：输出端 set/insert/remove/reorder、局部派生
  与显式化；保留顺序、长度与资源关系，持续合并同一变体的编辑。
- 时间函数（automation）：编辑参数/控制点，或在指定时间域覆盖输出；不能把任意函数
  自动当成有限音符数组展开，不能假设有限采样能精确复现曲线。
- 配置/绑定（音源、效果器、预设、路由）：改调用侧字段或引用，按 schema/defaults/
  automation target 修复关联，不对运行中的 DSP 有效值做源码反推。

每类都区分定义、生成调用、输出实例、循环轮次。选中输出时默认在离它最近的可表达边界
编辑；只有明确编辑生成器/定义，才将修改向上游传播。由此修改 chord/arp 无需在源码加 ID。
相同字段值相同不代表来源相同；新建对象身份与源值计算的“纯”也是两件事，新增 Pattern
可能消耗 ID 序列。有限集合转换仍须验证主方案里的概率和排序问题。

外部 TS factory 和 C ABI 插件同样参与上述配置/输出边界编辑，细则见
[外部插件与管理器方案](2026-09-08-plugin-manager.md)。需要单独区分公开参数与插件内部
生成结果；C ABI v1 没有 resources/state 传递和私有音符回读，不能泛化内置插件的编辑能力。

## 9. 后续原型与本次证据

将下列代表性例子加入源码写回原型，不能只用字面量 Pattern 或平铺八次作为出口：

1. chord 的单音音高/时长/力度/删除/补音；open、inversion、重复截顶音高。
2. arp 的每种 order、octaves、单步删除留空拍、插音不重排力度曲线、固定 seed。
3. chord → arp 组合，分别改和弦定义、arp 单步、某次放置、某轮播放；其他引用保持预期。
4. 24 个 automation 入口按上表走规则编辑与结果编辑；非可逆/退化条件不得猜测，
   chance 包装前后区间外样本对拍、loop/hold/tempo 限制与表达式预算均覆盖。
5. grid/onset 单切片改边界、切片排序影响 MIDI；grand/soft 单分区编辑及资源一致性。
6. fitToContent 手动拉边、零/饱和 velocityScale、效果器重排后的 automation 绑定。
7. 多轮编辑/Undo/外部改生成器/删缓存重新执行，普通 TS 输出能重建全部已保存音乐语义。

本次只修改方案文档。已运行现有纯 authoring 测试：
`pnpm --filter @oxitone/core exec vitest run test/chord.test.ts test/arp.test.ts test/automation.test.ts test/piano.test.ts`。
4 个文件、45 个测试全部通过；不打开音频设备。它们验证盘点涉及的既有行为，
不证明源码编辑器、来源追踪或上述新写回策略已经实现。其余结论来自列出的源码/协议审读。
