# Authoring 模型与双向编辑 API

本文件中的 API 是目标定义，需实现与迁移；不是当前 SDK 的用法说明。

## 1. 保留生成结构的 Authoring Graph

TS 求值产生不可变、可序列化的表达式图，Project builder 只组织引用和最终编排。
纯值构造不加载插件、不解码音频、不创建 engine。节点分为：

| 节点类型          | 内容                                                             |
| ----------------- | ---------------------------------------------------------------- |
| NoteSource        | literal notes、chord、arp、concat、repeat、slice、mapKnown、edit |
| ArrangementSource | placement、repeat、concat、window、occurrence edit               |
| AutomationSource  | 现有 source/组合器，加 replaceRange 与明确 time mapping          |
| PluginConfig      | definition 引用、物理初始参数、资源、配置 state                  |
| PluginInstance    | 独立实例配置；Channel 音源或 ordered insert 引用                 |
| SampleSource      | 原资产与非破坏性编辑链、切片/区域的生成表达                      |
| Binding           | typed parameter target、AutomationSource、优先级/时间域          |

内部节点 handle 自动生成，仅用于此文档执行与 wire 引用；源码只写变量和对象引用。
note/placement/slice/region 输出携带生成坐标和来源，不能提前只剩扁平数组。
wire 与 authoring 表达式用 tagged union 定义，禁止把闭包、AST 对象或任意 JS 函数传给 Rust。
chord/arp 等纯音乐生成在 TS 控制侧实现；automation 与音频资源解析由 Rust 实现。

## 2. 一个完整的创作例子

```ts
import { Project, chord, arp, arrange, plugin } from "oxitone";
import synth from "@acme/oxitone-synth";

const project = new Project({ tempo: 120, seed: 42 });
const harmony = chord(60, "major", { duration: 1 });
const phrase = arp(harmony, "upDown", 0.25);
const instrument = plugin(synth, { parameters: { cutoffHz: 900 } });
const lead = project.addChannel({ name: "Lead", instrument });
const track = project.addTrack("Lead").use(lead);

track.place(arrange(phrase, { at: { bar: 1 }, repeat: { count: 8 } }));
export default project;
```

`plugin` 接受安装描述，依赖注册由工程收集其引用后统一解析；不在构造时 dlopen。
源码里的 cutoffHz 必须由该插件声明。默认规则编排按 Pattern 长度重复，不假设固定小节长度。
Project 导出继续接受 sync/async factory，复杂项目可拆为多个 TS 模块。

## 3. 统一的局部输出编辑

所有有限集合表达式提供 `.edit(operations)`，返回派生表达式，不修改 base。
选择器在本次 edit 的原始 base 输出上解析一次，set/remove/replace/move 不改变后续选择对象。
目标选择唯一且类型合法才提交；匹配零个或多个分别报 EditTargetMissing/EditTargetAmbiguous。

```ts
const variant = phrase.edit([
  { select: { step: 3 }, set: { pitch: 65 } },
  { select: { step: 1 }, remove: true },
]);
```

`step` 明确表示这次 arp 结果的零基生成步，不是永远稳定的身份。删除第二步留下空拍，
第四步仍指原第四步；其余 start/duration/velocity 不重算，lengthBeats 不自动缩短。
插入使用 `{ insert: { pitch, start, duration, velocity } }`；新音符只增加该事件。
同一 selector 的互斥操作拒绝，同字段多次 set 在事务归约中合并为最后值；重复 remove 幂等。
set 和相对变换分开：`set:{pitch:65}` 与 `shift:{pitch:1}` 在上游 root 改变后意义不同。

不同 source 有类型化的选择器，而非通用“按 index 猜测”：

| source         | 可用音乐选择条件                                                       |
| -------------- | ---------------------------------------------------------------------- |
| chord          | degree/voicing 后的 voice；二者不同，编辑器选择正确层次                |
| arp            | 生成 step；也可明确 input occurrence + cycle branch + octave           |
| literal notes  | beat/pitch/voice 条件和同条件 occurrence；必要时针对该数组项直接写文本 |
| concat/repeat  | 子段位置、生成轮次、子 source selector                                 |
| arrangements   | 源放置位置/生成轮次，而非移动后的当前时间或显示顺序                    |
| slices/regions | 切片源序号/原 frame 边界；region 的键/力度矩形和样本引用               |

已知生成器优先保存语义坐标，例如 arp 的 step 或 chord 的 degree；这种规则自然跟随
root、rate 等上游参数变化，不因为旧 pitch/start 改了就让整首工程报错。源类型变化或
目标坐标不存在才拒绝。绝对 set 与相对 shift 的后续行为按各自语义执行。
对只能靠输出内容匹配的不透明生成结果，才附可读 `expect:{pitch:64,start:0.75}` 等约束。
这些约束不是隐藏 ID；匹配失效时由用户更新规则或固定数据，工具不悄悄丢弃约束。
未提交 GUI 事务另外携带旧值/source revision 前置条件，即使持久规则能重放，也不代表
外部代码同时变化时可以未经核对提交旧手势。不要把事务冲突检查全打印进长期音乐源码。

为了改过的 pitch/start 不改变随机事件地址，`.edit` 的输出继承 base 事件 origin；
不能把整个数组重排序后重新分配 origin。用户的任意 `.toNotes().map(...)` 是另一条通用
TS 路径，来源精度按边界能力标记，不承诺所有映射都能保持这种继承，详见 02/04。

## 4. 放置、循环与共享 Pattern

```ts
const arrangement = arrange(phrase, {
  at: { bar: 1 },
  repeat: { count: 8 },
}).edit([
  { select: { iteration: 2 }, move: { bar: 6 } },
  { select: { iteration: 3 }, replace: variant },
]);
track.place(arrangement);
```

重复条件在原 source 域解析，移动不会使后续编辑失去目标。实例替换默认保持放置窗口、
transpose/velocity/路由；用户可另选 fit-to-new-content。重复次数不因单次删除自动减少。
对单个实例修改音符，在该 placement 上引用 `phrase.edit(...)`，不改 phrase 定义。
对单轮播放改一个音，用 `{iteration, note:...}` 的层级目标在该轮挂接变体。

“拆分”创建源窗口视图，保留 source offset、原迭代号、随机 origin 和跨边界事件裁剪规则。
不通过重新创建三个无关 Clip 来近似模拟同一循环的前/中/后段。
拖动默认不 ripple、不交换、不覆盖其他 Clip；复制默认保留共享定义并产生新的放置关系。
Make unique 建立独立 authoring 变体；Make variation 才改变音乐 seed。

时间类型明确为 ProjectBeat、TrackBeat、PatternBeat、SampleFrame；公共位置用带 domain 的
结构或由对应 builder 固定域。bar 转换由工程拍号图定义；不在同一字段里混合秒和拍。
Pattern length 与 note duration、编排窗口长度分别保存；伸长音符越过窗口时显示截断，
用户可另做“延长窗口”，不在保存时偷偷改变循环周期。

## 5. chord / arp / 普通集合操作

chord 保留 root/quality/inversion/voicing 及原音级到最终声部的映射，截顶重复音不合并。
arp 接受 NoteSource 或显式 pitch/Note 数组；明确只用输入 pitch 生成节奏，其他属性由
arp options 决定。order、rate、gate、octaves、velocityCurve、seed 均是规则参数。
单音编辑在输出端，不能反推质量、改输入造成重新排序或影响同一音的多个出现。

新增受控 `concat/repeat/slice/transpose/velocity` 表达式操作；它们声明前向来源与编辑规则。
用户继续能用 JS 数组、map/flatMap/filter/sort 和普通函数；输出转换为 literal/opaque source，
或通过 `Pattern.generate(() => notes)` 显式建立生成边界。这个 callback 只在 Node 求值，
返回 serializable Note 数据，不是实时 callback；其内部无需提供逆函数。

需要退出生成抽象时，`Materialize selection` 只把选定输出写成普通 TS 数据。
该操作明确冻结当前生成结果，并声明是否冻结概率；不会自动把整首工程变成 snapshot。
Sparse edit 永远可用，不因例外数量阈值静默 materialize；编辑器可建议转换。

## 6. Automation 与全部现有组合器

保留现有 24 个入口的创作含义，改为保留可引用表达式节点：

- constant/curve/polyline/line：规则编辑改参数/控制点；line 增加折点转为 curve。
- gate/chance/wave，以及 sine/cos/triangle/saw/ramp/square：规则编辑保持生成器，
  局部绘制使用 source 或 lane 的区间覆盖；不反求 seed、不采样折线替换完整正弦。
- map/clamp/invert/quantize/scale/offset：共享输入与不可逆/退化条件均不自动反演。
- mix/add/multiply/min/max：用户明确选中分支才改分支，默认在合成输出侧编辑。

新增原生 `source.replaceRange({start,end}, replacement)` 与 `lane.replaceRange(...)`。
source 版本作用于 source local time，loop 后每轮重复；lane 版本作用于映射后的指定工程/
轨道时间，只改选定区间。两种范围不得通过一个模糊开关区分，见 04。
“只有某次播放改变”是临时试听，保存的 lane 编辑按可重放的时间/迭代条件表达。

## 7. 插件、路由与采样

每个音源/效果器实例都是一等对象，具有独立 typed parameter targets：

```ts
lead.param("level").automate(automation.line(0.2, 0.8, 8));
lead.instrument.param("level").set(0.6);
const echo = lead.addEffect(plugin(delay, { parameters: { feedback: 0.3 } }));
echo.param("feedback").automate(automation.sine({ periodBeats: 4 }));
echo.host.param("mix").set(0.25);
```

set 使用物理值，automate source 使用归一化值并由 parameter descriptor 映射。
源码通过对象引用绑定，wire 通过实例 handle + target kind，不再保存 `insert.0.parameter.*`。
效果器重排不改变绑定；删除时使其绑定成为事务内待处理关系，不能隐式转给下一个槽位。
预设/factory 配置是不可变值，`.withParameters/.withResources/.withState` 产生局部配置；
typed 插件参数接口和通用外部插件接口是相同语义，不强制第三方提供自定义编辑器。

Sample 保留非破坏性 edits 表达式；fitBars/fitBeats/fitToContent 是三种明确的长度约束。
手动拉边把长度切为所选固定域，其他 tempoSync/rate 保持。单实例改共享 Sample 时派生引用。
Slicer 的 grid/onset 通过 Rust 控制侧返回已解析切片与来源；拖动单边界用 slice edit，
插删时明确处理 trigger note 映射。region 生成也保留 grand/soft 的输入与默认音色设置，
单区修改通过 region edit；键/力度区域不重叠、资源引用和自动采样时钟由 Rust 验证。

现有所有音源/效果器 helpers、presets、Clock 与 routing 都映射到以上 typed 节点，
不能保留另一条直接写 Rust 内存、绕开 Document Service 的图形操作路径。
