# 执行语义、随机来源、automation 与换图

## 1. Authoring 表达与音频图分离

TS Authoring Graph 保留 source/生成/例外，Rust 接收可验证的纯数据表示：有限音符与编排
可以由 TS 在控制侧展开，必须保留生成坐标、源窗口与随机来源；automation 保留表达式。
Sample/Slicer 的解码、切片与时钟解析在 Rust 控制侧完成，返回有版本的显示投影。
Rust 编译成预分配的调度计划、DSP 节点、参数目标索引与资源 handles。

协议 2.0 的引用 ID 仅用于关联表，不再定义事件随机性、lane 顺序、混音顺序或 MIDI 顺序。
这些顺序用明确的 authoring order/priority 字段与拓扑规则表达。跨进程 ID 可以改变而音乐
不变；同时保留 generation，避免使用旧 graph 的实例/参数索引。

## 2. 新随机模型：不依赖实体 ID 或树路径

统一采用版本化 `oxitone-random-v2`，数值实现继续用固定 hash/PCG 算法与 TS/Rust golden
vectors，但 seed mixing 输入重新定义，不兼容旧的 clipId / AST child path 派生。

```text
randomDomain = H(projectSeed, sourceSeed, randomAlgorithm, purpose)
draw = PRF(randomDomain, musicalCoordinate, repeatIteration, transportIterationIfRestart)
```

sourceSeed 是正常的音乐 seed 选项，默认 0；purpose 区分 arp shuffle、Note chance、
automation chance 等随机任务。它不是唯一身份，也不要求每个实例不同。
生成器相同 seed 与相同坐标默认产生相同随机序列；复用/复制 source 默认保留序列。
需要独立变奏时用 Make variation 或显式 seed；GUI 写入新的数字 seed 作为可见的音乐设置。
不通过源码行号、变量名、内存地址、创建顺序或秘密缓存偷偷打散实例随机性。

| source                 | musicalCoordinate                                                   |
| ---------------------- | ------------------------------------------------------------------- |
| chord 结果             | 原音级与所选声部来源，voicing 排序保留映射                          |
| arp shuffle            | 输入序列坐标与 shuffle 步；输入集合变化是规则改变，会重新洗牌       |
| arp 结果的 Note chance | cycle branch、octave、原生成 step                                   |
| literal Note           | 创建时的 start/pitch/voice 等规范音乐坐标，不包含后续 edit 的最终值 |
| automation chance      | source 自身 decisionIndex；其位置不由表达式树路径命名               |
| repeat                 | source 坐标 + 原 source iteration，不用拆分后的新 Clip 序号         |

两个完全相同的 literal 事件且未区分 voice/seed，默认可共享随机样本；系统不伪造永久
身份来承诺它们独立。用户想独立变化可使用正常的 voice/seed/生成坐标。这个行为要在文档
和 UI 的“共享随机序列”提示中明确，不能让默认相关性成为隐藏实现细节。

`.edit`、范围覆盖、窗口视图和引用派生必须透传 base musical origin/randomDomain，
修改 pitch/start、移除其他事件、调整输出顺序都不重新编号 base 事件；insert 的 origin
由其显式音乐输入构造，后续 edit 同样透传。持续归约不得把它改成另一个 origin。
literal 的直接源码重写可改变被修改事件的 origin；无关音乐坐标不变的事件不受影响。
外部任意重写生成算法不承诺 origin 等价，视为用户改变规则；未提交事务仍须重新匹配。

同一概率阈值变动使用同一个 draw 比较，增加 probability 不随机洗牌其他事件。
包裹 automation source、移动 lane、调整未关联 source 都不改变 chance 轨迹。
ID/AST/创建顺序变化不改变 source；random restart 只在明确 transport play/loop 语义下变动，
seek/离线导出使用同一个可重建 evaluation context。

Materialize 随机生成的有限结果不能静默丢弃这种语义：保留 source/seed/origin 的视图
可以继续生成；若转成完全独立 literal 并烘焙一次概率决策，使用明确的 Bake range 操作，
声明冻结范围与放弃以后随机变化。普通 Save/Make unique 不执行 Bake。

## 3. 循环与窗口是源视图

Arrangement 和 Sample window 记录源域、offset、length、repeat 规则、原 iteration，
以及输出放置变换。裁剪/拆分只改变可见与调度窗口，来源继续指向同一 base。
整个事件实例的调度地址由 source origin + 原 iteration 确定；同一事件跨窗口不能被
重复发出 note-on。分割点上的 note-off/on 与跨界 sustain 行为必须由操作语义决定。

规定 Split 保持原连续播放结果：内部片段边界不强行断开跨界长音，编译时共享连续事件
来源去重；用户单独移动或删除片段后按新窗口裁剪，并给出跨界音符分配规则。
“切断音符”是另一项显式操作，会生成对应 note-off 和后段新 note-on。
Sample Split 保留读取相位和 stretch/repitch source window，不把后半段重播成素材开头。

所有范围使用左闭右开；同 frame 事件按 stop/note-off/parameter/note-on 与显式事件顺序
调度。Pattern 尾部截断、tempo/signature 变换、Track 独立时钟与 sample rate 换算按有理数
与整数 frame 实现；拖动时将显示坐标转换回选定 source 域，不保存 f32 累计时间。
MIDI 与 WAV 使用同一个事件展开器及随机 context，避免两条编译路径各自实现 repeat。

## 4. Automation 区间覆盖成为原生节点

新增 source 节点 `replaceRange(base, interval, replacement, boundary)`，不展开为一棵
乘加掩码树。base 是原节点引用，保留原 randomDomain 与 evaluation context。
默认 boundary=hard，采用 [start,end) 右连续规则；crossfade 是明确选项，带合法 fadeBeats。
区间外严格求 base，不以离散折线近似替换原函数。

source.replaceRange 的区间和 replacement 的 t=0 相对 source 本地时钟；lane.replaceRange
在 lane 完成 loop/offset/time mapping 后按声明的 Project/Track timeline 筛选，指定区间
只出现一次。replacement 的局部原点明确为 interval.start，base 时钟保持不变。
多个覆盖按显式 edit order 后写优先；归约可合并不重叠区间，不重排改变含义的重叠项。

lane schema 改为独立的 sourceTime、activeRange、loop、afterEnd（hold/stop/reset）和 priority。
不继续以 lastBeat 同时暗示 loop 截止与值保持。编译时校验每种组合，默认行为在 API 文档
明确；同 target 多条 lane 按 priority + authoring order 合并，不按 opaque lane ID 排序。
source 的所有组合在归一化域运算，最终映射与范围校验由已验证 ParameterSpec 决定。
参数默认值、源码显式值、临时试听、automation 和 smoothing 的优先级统一实现。

确定采用：普通写配置改 base；处于 automation 期间的 live preview override 是手势期间
的临时最高优先级 physical value，结束后写 source/base 或指定 lane 并撤销 override。
它不自动录制 automation；Write automation 模式才按明确时间域采样/归约写入曲线。
不能把试听值永久留在 engine 中而 TS 没有对应修改。

保留 TS/Rust 的深度/节点/事件/烘焙范围预算，DAG 共享不重复复制同一 source；语义求值
若依赖不同 context，不能因共享 node pointer 错误复用结果。超限报告具体 source，
不自动把曲线烘焙成更低精度数据。tempo lane 必须 transport-invariant，不允许 chance
或 restart；有效 BPM 曲线继续在控制侧烘焙，再统一影响整个工程的 beat/frame 转换。

## 5. 增量更新与实时图所有权

把变化分为参数、配置状态、结构、资产和纯呈现：

- 参数试听/已支持平滑的参数提交：固定容量事件队列，带 graph generation/target index。
- 配置 state、插件版本、资源或路由变化：控制侧验证与 prepare，发布候选 graph。
- 不变的 DSP 实例可在边界受控转移所有权以保持 voice/tail；不能让两个候选图并发 process
  同一个实例，候选验证也不能 mutate 正在播放的实例。
- 无法安全复用的变更重建该实例并清晰定义 reset/tail；保留不受影响节点与 transport。
  跨版本插件没有可迁移 DSP 状态时不承诺声音完全连续，UI 显示其 reset 行为。
- 纯代码位置、布局、目录刷新不重建音乐图。插件管理只增加未使用条目也不重置播放。

source session 对已验证的同一实例做跨版本 handle reconciliation，仅用于会话内复用。
无法唯一匹配就新建实例，不能为了无缝播放把旧状态交给另一个插件。
source identity 的不确定性不影响保存的随机音乐结果，因为随机模型与 handle 分离。

音频回调及 process/reset 保持 allocation-free、lock-free、non-blocking、无 I/O/JS/JSON。
准备、移交表和回收队列都提前分配；block 边界只做有界交换，旧 graph/资源/库在控制侧销毁。
候选失败、队列满、过期 generation 不替换当前合法图；graph-active ack 与 horizon 对齐。
parameter-only edit 也必须在最终 Save 时验证源码重开配置，不以临时声音正确替代持久化验证。

## 6. 平台和数值

native GPUI、N-API 与 Wasm 复用相同 graph、参数寻址、随机、事件展开和 automation evaluator。
浏览器 worklet 只搬 PCM；Worker 不执行未协商动态 native 插件。平台不支持插件时明确报错，
不能自动换内置音源。导出声明实际能力与依赖，不能假设所有平台二进制通用。
sample decode、SRC、stretch、PDC、denormal、稳定求和和 dither 的既有数值约束继续适用；
新随机与排序模型建立新 golden，不要求保留旧 ID 驱动的字节结果。
