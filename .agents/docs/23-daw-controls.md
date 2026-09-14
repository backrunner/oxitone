# DAW 工程控件与编排事务

本增量延续 Document 2.0、Engine 1.2 与 Plugin ABI 1，补齐日常编排的操作入口。
它不代表 source/DAW 设计矩阵全部完成。未实现的运行时与资产能力仍见 [18](18-project-daw.md)。

## 工程事务

`Project.configure(edit)` 支持 Channel/Bus level、pan/balance、mute、solo，Track enabled/mute/solo、
静态 Project tempo，以及 instrument 替换和串联 insert 添加、替换、删除、重排。
Document `project` operation 先校验 baseRevision，再使用捕获的最终 Project return/default export。
索引对应 builder 创建顺序，不对应 canonical snapshot 的 ID 排序；投影包含 channels 和
mixerChannels 的顺序。GPUI 使用稳定执行引用查找当前索引，不将执行 ID 写入 TS。

```ts
export default project.configure({
  kind: "channel",
  index: 0,
  values: { level: 0.7, pan: -0.2 },
});
```

原表达式只执行一次，原有 import、export 与局部变量保持。相邻同一目标的纯 JSON scalar
configure 合并字段，不增加多层括号；含注释、动态值或未知调用的源码保持原表达式。
不合并插件替换、移动到其他 Track 等会改变后续身份／索引含义的结构操作。
同一 owner 的相邻纯 JSON `effectOrder` 合成为一次排列；实例和 automation 绑定保持。
GPUI 重排通过当前 builder 顺序调用工程事务，也覆盖在最终 Project 表达式中新增的效果器。
无状态变化不产生源码补丁或新文档 revision。所有参数范围在首个 builder mutation 前验证。

候选在创建顺序中从 accepted snapshot 恢复并计算完整 expected snapshot；真实代码重新求值、
原生 compile、全工程等价比较和依赖读集复核全部通过后接受。只有新分配的 clip/plugin instance
身份可建立别名；已有身份、自动化绑定、其他轨道和工程设置必须一致。Save、Undo/Redo、外部
编辑器未保存同步沿用同一文档，不直接设置音频线程或另存工程状态。

## Mixer 与传输栏

Mixer 音量推子／声像可拖动，Shift 精调，双击恢复 0 dB／中央；M/S 为可操作按钮。
拖动期间仅显示本地值，释放后提交一条事务；Escape 或外部 revision 变化取消活动手势。
pending 由对应 response 清除。静态值受自动化覆盖时仍遵循原生 automation combine 语义；
本增量不增加参数试听或 automation touch/latch 模式。

Track 名称单独一行，第二行提供 M/S 快捷按钮，色条切换 enabled。Track mute/solo 是
Engine 1.2 的可选 boolean，默认 false；只过滤本 Track 的 Pattern、Sample 和 Playlist
automation placements，不改写共享 Channel。Mute 优先于 Solo；多个 enabled Solo Track
共同播放。disabled Solo 不压制其他 Track；只剩被 mute 的 Solo Track 时播放静音。
M/S 不缩短时间线或改变 tempo 烘焙范围。GPUI/N-API 播放尊重 Track/Channel/Bus Solo；
WAV 导出通过现有 respectSolo 决定是否尊重，默认忽略；MIDI 忽略 Solo、排除 muted Track。
修改仍经 Document 事务接受、撤销和保存，连续 Track configure 合并各字段，不互相覆盖。

传输栏移除 Locate 输入及 G 快捷键，使用时间轴点击、marker、Home、方向键设置 cue。
BPM 数字可直接输入，Enter 提交，Escape 或点其他区域取消；
存在 tempo map 多段或 tempo automation 时保持读数，必须编辑已有 tempo 定义，不能静默覆盖。
输入期间数字／Space／字母不会触发播放或编排快捷键。独立 Preview 没有文档连接时保持只读。

## Playlist

Pattern、Sample、Automation placements 共用选择、Shift 拖动复制、⌘D 在选择末尾复制、
Delete/Backspace 删除、M 启停。拖动按当前 viewport/lane bounds 查找落点，Alt 使用 1/960 beat，
默认 1/4 beat；复制保留完整 placement 配置，共享原音乐定义，创建独立 placement 身份。
同 Track 移动不改变 builder 顺序；Pattern lastBeat 随起点平移，保持 exclusive end 与长度。
拖动使用片段本身的名称和内容缩略图，不保留空框占位。跨轨移动只显示一个片段，Shift
复制保留原片段；资源拖入也使用相同 renderer。缩放边缘实时更新内容的可见长度。
无效落点保留原片段，取消不写代码。松手后在旧 accepted snapshot 上保留最终位置，
直到匹配的 accepted snapshot 接替；失败恢复旧状态，不在确认过程中跳回原位或重复显示。
钢琴窗绘制和命中检测均使用 SourceSite outputs 的相同顺序，重复音符按输出索引区分；
移动、复制、时长和力度编辑直接显示音符本体，鼠标释放位置参与最终计算。pending 音符
保留提交时的输出列表，只投影到对应 pattern 和旧 snapshot，接受后恢复选择。

右边缘调整 Pattern／Automation 时长。Pattern 已有 lastBeat 时更新结束位置，否则更新 duration。
Sample 边缘使用独立 `fitSample` operation，沿用当前 tempoSync：off 是播放窗口，stretch／repitch
按既有引擎规则适配 beat 时长；标签明确显示 AUDIO／STRETCH／REPITCH，不修改 sample 原文件。
带局部 Track tempo 的移动、复制和长度调整暂时拒绝，避免把 project beat 当作 Track beat。
相邻同片段的 resize/fit/enable 调用合并，无变化不落事务。全局显示留有尾部编排空间。

此处未提供左边缘源偏移、保持相位的 split、多片段框选／剪贴板或跨 clip automation 绘制；
它们不能通过复制后裁切冒充，仍依赖 musical origin／source window 的后续执行语义。

## 钢琴窗与内部窗口

Automation 目标导航与 Browser/Playlist 共用展示模型。Channel 的 `level` 显示 Volume，
插件参数使用已接受 descriptor 的 label；typed instance 与 owner insert path 解析到同一实际
插件分组，重排更新插槽名而不改绑定。Host mix 显示 Dry / wet，区别于插件自己的参数。
重复目标名称与同目标重复参数在选择器中补充序号，内部 key 始终来自实际身份，不能按显示名绑定。
单目标时直接显示归属和参数按钮，多目标增加一行目标切换；Browser 中归属为较弱的第二行。
仅调整显示，不写入实体 ID、不更改 automation AST、范围或 source/DAW 事务。

钢琴窗以独立的 beat 像素尺度显示，默认 96 px/beat；网格不受 Pattern 当前长度裁切。
横向虚拟范围随滚轮、平移、键盘、滚动条与音符移动继续增长，只绘制视口内的行与刻度。
接受新的 Pattern 长度不改变缩放或偏移；F 适配内容并留出至少 8 beats 的绘制空间，
End 到达 Pattern 尾部。全部 128 个 MIDI 音高通过纵向滚动到达。
音符右边缘按 `start + duration` 使用网格的同一坐标映射，不额外扣除尾部间隙；
缩放后仍与命中范围一致，极短音符保持 7 px 的最小可见宽度。显示不量化或改写源音符时长。

插入、绘制、拖动、复制、量化和时长调整可超出原长度；Document 将新增／时间发生变化
的音符末尾向上取整到整数 beat，必要时用 `.edit([...], { lengthBeats })` 延长，或更新
本地 literal。没有强制四拍小节假设。自然长度／loopCount placements 使用新周期；显式
duration／lastBeat 仍裁切在用户设置的片段边界。共享和单引用编辑沿用当前选择范围；
npm 输出改动仍在当前项目，materialize 同时保存长度，不修改依赖文件。

Velocity 区左键直接按绝对高度设值；横向拖动在鼠标事件间插值，扫过的同起点音符
一起改变。有选择时只绘制选中的音符，接受后保持选择；无选择时后续笔画仍可覆盖全部。
每组同起点竖线共享 24 px 的横向目标，随相邻起点缩窄，不受组内音符时长差异影响。全程投影实际音符／力度柄，释放
只提交一次，Escape 取消，外部 revision 取消活动手势，等待接受时保留最终图形。
力度从底部 0 到顶部 1，音符填充不透明度为 0.22 + velocity × 0.73，零力度仍可识别和
选中，选中颜色同样使用该透明度。右键力度区不会删除音符。

全部内部浮窗共用 8 px 圆角壳体、7 px 标题栏内圆角。GPUI 当前仅支持矩形内容 mask，
因此内容在底部保留 7 px 壳体边距，避免 Canvas 或滚动背景覆盖圆角；八方向缩放保持。

## 插件选择与实例

Mixer Chain 使用紧凑行：点击名称打开实例面板，替换按钮打开 Library picker，删除按钮移除效果；
底部 Add effect 添加 insert。Library 仍只展示名称／类型，选择目标时仅显示匹配类型，Enter 或
Add/Replace 提交 `assignPlugin`，成功后返回 Mixer；失败保留选择与诊断。资源依赖或复杂状态
尚未提供完整创建向导，原生 compile 拒绝缺失资源，不能作为成功添加处理。

选择以 owner 和可选旧 instance 引用寻址；回写仅使用 builder index/slot。替换产生新实例，其他
实例身份保持，已有 typed 或 legacy automation 绑定禁止隐式迁移或移除。配置编辑与面板窗口按
instance 身份跟踪，重排仍显示原插件；替换／删除后旧面板显示未附着，不自动控制新占位实例。
`configure` 在验证后保留原输入配置对象的来源关联；新增插件的配置通过捕获的 Project 边界
定位，并以 runtime usage 与完整候选校验确认。内置／npm 插件均可在添加后继续编辑参数。

外部插件使用同一入口：独立 helper 验证 library/manifest/hash 和既有签名策略，检查 metadata、
库、模块与 lockfile 读集；新增注册写为项目本地 `.withPluginRegistration(...)`，库路径基于
`import.meta.dirname` 与正常 node_modules 包路径，不写入 pnpm store 实际路径。
已有注册保持不变，同 ID/version 不允许不同 hash 冲突。expected frame 只允许增加这份精确注册，
其他 plugins、plugin UI、allowPlugins、assetBaseDir 继续校验。默认 signed-only 不自动降级。
音乐操作不修改 node_modules；显式 package 生命周期任务保持独立。

安装／修复可在 idle invalid 草稿下运行，失败恢复原 status，成功重新求值，使缺包工程可恢复。
无效草稿不能因此获得音乐编辑权限；完整 package store rollback、ABI 2 resources/state、并行
rack 与宏迁移仍未完成。
