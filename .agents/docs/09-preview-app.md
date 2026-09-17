# Oxitone Preview App（GPUI Viewer）

**目标变更**：已批准升级为 [GPUI DAW 与 Node Document Service](../designs/source-daw/README.md)，
包含音符/编排/automation/插件编辑和插件管理器。`oxitone daw` 已接入音符编辑、作用范围、
拆散 review、Automation source 绘制、插件初始配置与串联效果链拆散、Undo/Save、editor bridge、冲突解决及插件目录/验证，见
[18-project-daw.md](18-project-daw.md)。下文描述 `preview` 只读模式；“不提供编辑”
不再约束新增能力。所有编辑必须走文档事务、来源校验与 accepted revision，禁止 GUI
直接写源码或实时图。引擎拒绝候选时保留上一可播放版本。

## 定位与边界

Preview app 是 Oxitone 工程的可视化呈现器：**代码是音乐的唯一事实来源，viewer 只读**。用户可以 play/pause/stop/seek/loop，可以调整 viewer 自身的显示偏好（缩放、主题、scope 开关），但任何操作都不会反向修改代码或 authoring 数据。viewer 内不提供任何编辑能力（无拖拽音符、无画 automation、无推子写回）。

与代码的同步是单向数据流：

```text
TS/import 代码变更 -> esbuild 单文件 ESM -> runner 执行产物 -> ProjectSnapshot(revision+1)
            -> IPC -> viewer 编译（控制线程）-> block 边界换图
engine 状态 -> meter/analysis ring -> viewer 呈现
viewer 操作 -> transport command -> engine（仅此一个方向回到引擎）
```

## 进程架构

两个进程，职责严格分离：

- **runner**（Node，`@oxitone/cli` 的 `preview` 子命令）：加载并 watch 用户的 TS 工程入口（含其 import 依赖图），变更时防抖（默认 150 ms）重新执行，产出带单调 `revision` 的 `ProjectSnapshot`，通过本地 IPC（长度前缀二进制帧，复用 `protocolVersion`）发给 viewer。TS 执行错误、schema 校验错误原样转发为诊断帧。
- **viewer**（`oxitone-preview`，GPUI 二进制）：直接链接 `oxitone-graph/render/mixer/io-macos` 等 Rust crate，**不经过 N-API**；自己持有 CoreAudio 输出和 render-ahead ring。snapshot 编译在控制线程，换图复用引擎现有的原子发布机制。

进程分离的理由：runner 崩溃或代码报错时 viewer 继续播放上一张合法图；viewer 重启不要求用户重新组织代码；音频路径上没有任何 JavaScript。

## 同步语义

入口默认导出 Project 或返回 Project 的 sync/async factory，也可返回
`{ project, assetBaseDir? }`；Project.registerPlugin 的已校验库配置随 IPC 发送。
runner 用 esbuild 合并本地静态 import/export、JSON 和可静态解析的 dynamic import，
每轮生成一个带 inline source map 的临时 `.mjs`，独立 Node 子进程只执行这个产物，
不再执行原始入口。各模块 import.meta.url/dirname/filename 由 AST 转换保留为原文件
位置；npm dependencies 依据源文件包作用域解析为 external file URLs，Node builtins
保持 external。这些本地开发产物仍依赖已安装的 SDK/native 包、采样、dylib，不能
宣称为可跨机器移动的完整分发包。相对资源目录通过保留的 `__oxitoneSourceDirectory`
导出携带；显式 assetBaseDir 优先。该导出名由 bundler 保留，工程勿自行定义。
`oxitone build <entry.ts> [-o project.mjs] [--watch]` 使用相同打包器输出单文件；
成功后原子发布，失败保持上一个文件，拒绝覆盖入口或依赖源文件。Preview 可直接
加载该 `.mjs`，原始 TS 模块已被合并，不再需要用于执行。每轮执行有 10 秒超时、64 MiB
结果上限；代码日志走 stderr，snapshot 单独走 pipe，不混用 stdout JSON。
运行期动态 import/文件读取依赖可由 `--watch-path <path>` 显式补充。未使用文件扫描
或远程下载来寻找插件。

IPC 是仅本机 Unix socket，目录 0700、socket 0600；4-byte big-endian 长度 + UTF-8
JSON，单帧最大 64 MiB。`preview-frame.schema.json` 定义 snapshot（含 revision）、
diagnostic、status、transport、query、shutdown；所有帧含 protocolVersion，当前为 1.2。
`preview-response.schema.json` 定义有序 state/rejected 应答；runner 一帧 in-flight，
队列保留各类型最新状态，避免慢编译积累大快照。native rejected 清除该 revision 的
去重缓存，下次 watch 事件允许重试相同内容。只有 snapshot 能替换 authoring 呈现，transport 仅改变播放状态。viewer 断线仍持有
最后合法图；runner 显式退出才发送 shutdown。`--headless` 使用模拟输出供集成测试。

- revision 单调递增；viewer 忽略乱序或重复 revision。
- snapshot 与独立 pluginUis 内容 hash 不变时不发送新版本（runner 负责 diff）。
  仅 UI 变动时 viewer 复用相同音乐源的 graph/telemetry/音频实例，只发布新呈现；详细校验与回退见 `11-plugin-ui.md`。
- 编译/校验失败：保持当前可播放图（引擎既有语义），viewer 显示结构化诊断 overlay，包含稳定错误码和 JSON path。
- 换图在 block 边界完成；正在播放时换图不中断 transport，seek/loop 状态保留。

## 视图

普通 `preview` 是只读音乐投影；`daw` 接入 Document Service 后提供 [18](18-project-daw.md)
的语义编辑、撤销和保存。两种入口共用窗口、导航和 transport。

- **Playlist/arrangement**：固定 Track 侧栏与小节标尺，PatternClip 的彩色标题与真实 note 缩略图；缩略图遵循 Track tempo、clip 重复和截断。首次打开适配工程长度，markers 独立导航。未命名 Pattern 按全局首次出现顺序编号，同一 Pattern 的多个 clip 共用名称，派生标签按 snapshot 缓存。
- 编排区按 Track 展示；Track 是乐曲时间线容器，可以叠放多个 PatternClip、SampleClip 和 Automation 片段。Channel 负责乐器与 Mixer 绑定，Pattern 声部选择各自的 Channel。资源浏览器以紧凑行提供可拖入 Playlist 的 Pattern、Sample 与 Automation；拖放、移动和删除通过 Node Document Service 的单一语义事务写回源码。Browser 只保留资源名称与长度，不常驻显示 placements 详情或操作教程。Automation 片段显示参数归属和原生求值的本地曲线缩略图。
- **Piano roll**：完整 128 MIDI 音高范围，固定琴键、局部 Pattern 拍标尺和 velocity lane；自动适配选中音符，播放头及 native note-on/off 高亮。音符与琴键可滚动、缩放；DAW 的音符手势通过 Document Service 回写。
- **Mixer**：紧凑通道条、固定 Master、通道颜色、level/pan/mute/solo、分段 peak/RMS 表（两列不是 L/R）；DAW 控件通过 [23](23-daw-controls.md) 写回，preview 保持只读。独立可收起的效果器/路由 inspector 展示完整名称、bypass、send 和 Master scope true-peak。
- Mixer 详情采用 Chain/Routing 标签：紧凑插件行提供打开、替换、删除，底部添加效果器；mix/bypass 位于实例配置。路由卡片显示直接输出、独立 Master 比例、aux/sidechain sends 和输入来源，含比例/dB、实际 pre/post tap 和自动化标识。Channel 只拥有直达 bus 的输出；其下游 send 标为 “Sends via …”，不冒充 Channel 自己的 send。侧链遵循引擎固定 post-insert/pre-fader detector tap，不进入目标音频求和。
- 通道条显示输出目标与 send 数；选择连接跳转并显示目标通道，关联输入/输出标为 IN/OUT。Inspector 按宽度适配，Mixer 可展开到整个编辑区并恢复 Split。所有导航仅改变视图；路由模型随已接受快照缓存，换图更新，拒绝换图保持上次合法连接。
- **插件详情窗口**：Mixer 的音源入口、Channel/Mixer/Master 的每个效果器实例均可独立打开。窗口按稳定实例身份复用；不同实例可并排查看。插件参数绑定使用 EffectRef.instanceId；重排后详情仍跟随原实例，替换或删除后显示未挂载状态，不自动控制新占位插件。无实例身份的只读投影保留槽位寻址。
- 详情来自已接受快照与控制线程复制的权威 descriptor，包含参数显式值/默认值、物理范围/单位、平滑/rate/mapping、自动化绑定、效果器 mix/bypass、音源资源/structured state、布局/复音/capabilities、内置或动态库来源及已验证 hash。参数是 source 初始配置，不能冒充自动化/平滑后的实时有效值；不创建第二个 DSP 实例、不调用插件 process/getter、不持有动态库或 DSP 指针。
- **Scopes**：波形（各 bus 峰值 ring）、频谱（FFT）、相位空间 XY/vectorscope（M/S 分解）。
- **Transport 条**：play/pause/stop/seek（bar/beat/marker/点击时间轴）、loop region、当前 bar.beat.tick 与 timecode、tempo 显示（含 tempo lane 烘焙结果）。

## 分析数据路径（实时纪律）

- audio 线程只写预分配 ring/atomic：meter 值、降采样峰值、analysis 帧、transport 游标。**FFT、加窗、M/S 分解都在 viewer 的 UI/分析线程执行**，从 ring 取数。
- UI 以约 30 fps 节流消费；满 ring 丢新 analysis 帧并计数，transport ack 不允许丢（沿用引擎规则）。
- 播放头显示必须补偿 `getOutputLatency()`：画面位置 = engine 游标 − 输出延迟，使视觉与听觉对齐。
- viewer 显示 `engineLoad`、underrun 计数和插件 fault 归因，与 `05` 的诊断指标一致。

## 交互与延迟

- 窗口内容延伸到顶部，系统标题文字/独立标题栏隐藏；44 px 标题区放工程名与编辑器导航，
  标题只使用真实工程名（未命名为 Untitled）及修改标记，不放 slogan 或重复品牌。
  48 px 传输栏集中播放、位置、BPM 与 Undo/Redo/Save。保留原生 macOS 交通灯按钮。
  非交互标题区可拖动，双击遵循系统标题栏偏好；全屏时收回交通灯预留空间。
- 标题区将 Arrange / Piano / Mixer 作为独立的图标与文字主视图组；右上角只保留
  Automation、Plugins、Browser 和快捷键图标入口，使用悬停名称、选中态与分隔线区分层级。
  插件面板只显示一条固定的声音分页导航，参数检查、资源及插件信息收于 Inspect 入口。
- 外观始终跟随当前窗口的系统 Light/Dark（含 Vibrant）appearance，启动时读取并
  订阅运行中的变化，无须重启。轨道、钢琴窗、Mixer、Scopes、诊断、按钮 hover/active
  和编辑控件 focus 共用语义配色；主题不进入 ProjectSnapshot，不触发编译或音频命令。
- viewer 的 transport 操作走引擎同一 command queue；生效延迟 = ring horizon（见 `03`），UI 据此做预期反馈（按钮立即响应，播放头按 horizon 对齐）。
- seek 目标支持 bar/beat/marker/timecode 与时间轴点击；点击位置按当前有效 tempo map（含烘焙的 tempo lane）换算。
- Playlist 标尺、clip/空白轨道与钢琴窗标尺/音符区/velocity 区单击精确定位（不吸附刻度），双击或 Option/Alt 单击从该位置播放；播放中单击保持播放。钢琴位置按 Track tempo 转为全局时间，并跟随当前 clip 的重复轮次，截断部分夹紧到 clip 边界。定位到循环区外会关闭循环，避免播放跳回旧区域。
- 鼠标点击时间线、钢琴标尺或 Marker 设置播放起点（cue）；双击／Option 点击从该处播放。Space 播放/暂停，Enter 从 cue 重播，Shift+Space/Stop 停止并回到 cue。Option/Alt+←/→ 逐拍定位，加 Shift 按当前拍号移动一小节；⌘/Ctrl+Home/End 到工程首尾，[/] 到前/后 Marker，L 切换循环，? 打开快捷键说明。
- 音源/效果器详情窗口共享播放、停止、逐拍/小节和 Marker/循环快捷键，不抢占原有无修饰滚动键。播放类切换忽略键盘自动重复，连续定位键可重复；所有交互只发送 transport 命令。播放按钮/定位标记先显示请求状态，native 状态到达后校正，播放头仍来自引擎。
- 启用循环时若当前位置在区外，先定位到循环起点。暂停/停止后的定位不减输出延迟；只有播放中采用 audible frame。首次 play 前的有效 watch 换图同样保留 frame cursor/state/loop。
- Playlist 支持双轴滚动与拖动滚动条，Shift 滚轮横向、⌘/Ctrl 滚轮缩放，Fit 恢复全工程范围；长时间轴按可见范围创建标尺刻度。
- Piano 支持双轴滚动与拖动滚动条，Shift 滚轮横向、⌘/Ctrl 滚轮缩放时间、加 Shift 缩放音高行高；两轴缩放均以指针为锚点。中键或 ⌘/Ctrl+Alt 拖动平移，Fit 恢复适配。点击后方向键、Page Up/Down、Home/End 移动视口，± 缩放、F 适配；点击局部拍标尺 seek，Loop clip 使用全局 clip 边界。
- Mixer 滚轮/触控板与 ‹/› 按钮横向浏览；面板过矮时 Alt 滚轮或纵向滚动条浏览下部；Master 同步纵向位置。点击后左右/Home/End 选择并显示通道，上下/Page Up/Down 纵向浏览；inspector 独立纵向滚动。选择只改变分析 scope。
- Playlist/编辑区、Piano/Mixer、Mixer inspector、钢琴 velocity、分析区使用 8 px 命中分隔线。
  手势在窗口 capture 阶段跟踪，跨 sibling hitbox 后继续，尺寸按实测工作区计算，不扣固定标题高度。
  Velocity 分隔线在 prepaint 使用本帧实际画布高度计算命中区与拖动上限，切换独占/停靠时
  不沿用上一帧的小面板坐标或上限；绘制与手势使用相同的力度区夹紧规则。
  Arrange 默认上方编排、下方整宽钢琴窗；编排栏的停靠工具可切换整宽 Piano/Mixer、并排
  或再次点击收起。缩放控件位于各自编辑器。F5/F7/F9 或最大化图标切换编排/钢琴独占/Mixer 独占；
  恢复保留停靠配置、尺寸与独立滚动/缩放。独占指完整编辑工作区，可配合原生窗口全屏；
  不是新建第二个文档或 DSP 实例。Analysis 默认折叠，不占工作区行，通过底栏波形图标展开；Mixer 详情默认收起。
  Space 播放/暂停，位置输入框独立处理文字。所有视口状态仅在 viewer 内持有，watch 换图保留仍有效的选择并按新边界夹紧视口。
- 详情由 GPUI Entity 嵌入主窗口的统一内部窗口框架，使用公共标题栏和宿主 appearance；支持参数筛选、滚动、参数/资源/插件信息切换和复制该引用的 JSON。重复打开聚焦已有窗口；独立关闭不影响播放或其他窗口，关闭主窗口退出整个配对 session。有效 watch revision 才更新详情，失败编译保持上次合法数据。声明式自定义 GPUI 面板通过 Project.registerPluginUi 注册，详见 `11-plugin-ui.md`；不执行插件 UI 代码。
- DAW 有未保存修改或在途操作时，原生关闭与 ⌘Q 打开保存确认。Cancel 返回编辑；
  Close without saving 在操作结束后可用；Save & close 必须收到自身 Save 成功应答，且
  同一 session/revision 已保存、无在途操作才退出。保存失败或期间代码发生变化保留窗口与草稿；
  Cancel 仅取消自动关闭意图，不撤销已经提交的 Save。进程强制终止不属于窗口关闭保护。
- 快捷键帮助和关闭确认在根捕获阶段隔离键盘及指针，阻止底层音符、Undo、播放和内部窗口快捷键，
  并取消活动手势。源码错误与 native 图诊断分别持有，迟到的 native Accepted 不清除源码失败；
  底栏区分 Saved、Modified、Syncing、Updating audio、Saving、Invalid code、Conflict、Error 与 Disconnected。
- Browser、Piano、Mixer、Automation、Plugin Library、实例配置和插件面板共用一个内部窗口管理器。
  点击置前、标题拖动/双击最大化、八方向缩放、恢复/关闭和 Piano/Mixer 停靠均在同一 GPUI
  主窗口内处理，浮层占用实测 desktop，保持主传输栏可用。Arrange 隐藏浮层，再次打开
  编辑器恢复其状态；⌘/Ctrl+W 关闭当前内部窗口。手势命中不穿透窗口；Playlist 拖放还验证
  滚动视口边界，并在释放时重新计算目标。`OXITONE_PREVIEW_CAPTURE_WINDOWS=1` 配合
  `scripts/smoke-daw.mjs` 验证六个内部视图共存、真实标题/边角/最大化按钮与 Browser 拖放保存，
  同时断言系统窗口数量为 1。完整音频及来源契约见 [18](18-project-daw.md)。
- 全部 26 个内置效果器与 4 个音源的 Panel 页提供专用分组和原生参数响应图，窄窗口重新排列图形与控制组。
  DAW 可拖动旋钮/fader、选择模式及切换 host Mix/bypass；Shift 精调、双击恢复默认、Escape 取消。
  手势即时投影参数/图形，释放后经 Document Service 提交一次实例配置，支持 Undo/Redo 和源码保存。
  图形基于 source/default，缓存在 UI 侧，不访问 DSP 实例；具体图形语义和第三方覆盖见 [11](11-plugin-ui.md)。
  Inspect 的紧凑行表显示全部参数，Specs 展开 ID、mapping/rate/smoothing 和完整自动化绑定。
  钢琴标题/编辑工具分两行，窄面板工具可横向滚动；正常宽度分屏保留 Piano ≥360 px、Mixer ≥340 px，
  极窄窗口按比例夹紧。界面使用中性色分层、克制边界与选中状态。
  插件库统一为 30 px 列表行：名称与 Instrument/Effect，直接 All/Instruments/Effects 筛选及搜索。
  同名条目补充 vendor/version 以区分；不可用插件保留异常提示。单击/方向键只选择，
  Used in project 与 Details 按需展开；不常驻参数、计数、卡片或技术元数据。
  包安装入口保留于顶部，验证/修复/升级/卸载与来源信息位于 Details。
  实例配置通过使用位置 Edit 或插件面板 Configure 打开独立窗口，标题标明归属和槽位；
  浏览插件库不改变正在编辑的实例。配置保留初始值输入、作用范围、Mix/bypass、重排和拆散 review。
  配置窗口初始高度随参数数量夹紧，重开同插件保留用户尺寸；库详情随窗口高度缩小，保留列表空间。
  控件分普通、主操作、无背景和图标按钮，工具用分组容器。图标有原生悬停名称/快捷键；
  钢琴/曲线底部仅显示音乐数据，不常驻操作教程，所有快捷操作集中于 ? 面板。
  Pattern 作用范围和 Detach 位于钢琴工具栏，传输栏只保留全局文档操作。
  编排/钢琴标题不重复统计数量；插件面板仅在非同步状态显示诊断标签，Copy JSON 归入 Info。
  Mixer 隐藏 0 FX 与静止的负无穷 peak；Automation 移除内嵌卡片边框和重复插值标签。
  28 px 底栏显示保存状态、CPU 和必要引擎告警，采样率/延迟/revision/xrun/fault 点击展开。
  布局参考 FL Studio 的编辑器工具栏、Live 的上下编排/详情层级与 REAPER 的停靠 Mixer；
  不增加未实现的录音或资源导入入口；已接入的 Mixer 控件见 [23](23-daw-controls.md)。

## 打包与启动

合成器面板按 Oscillators / Filter & output / Modulation / Matrix 四页分区：A/B 独立 octave/level、
Sub 六种波形及独立 octave、bank/warp 周期图、FM/ring、三组曲线包络、两组 LFO
和八槽矩阵/四个 macros。新增 `subOscillator` 与 oscillator 可选绑定详见 `11-plugin-ui.md`。
图形基于 source/default 和共享波表，不表示播放中 automation/modulation 的有效输出。
模块使用统一的标题带、细边界和控件底面，选择器单独横排、连续旋钮按列排列；
滤波与输出入口不再藏在振荡器长页末尾。移除重复的路由读数、波形底部参数摘要与大写标题。
插件窗口标题优先显示插件名称，再显示所属 Channel/Bus，效果器补充插槽编号。
Source controls 页名说明初始参数语义；Configure 收入页签栏，移除重复的 Initial settings 行，
异常同步状态才占用额外诊断行，保留 host Mix/bypass 的固定入口。
Automation 编辑器按目标分组后选择参数；同一目标只显示一次归属，不把 `level` 等代码路径
直接当作按钮标题。Browser 使用参数名称与归属两行，Playlist 使用参数优先的简短标签，
Channel Volume、Bus Balance、send 目的地、插件 descriptor 名称和 host Dry / wet 分别解析。

- `oxitone preview <entry.ts>`：CLI 启动 runner，并拉起/连接 viewer。
- `pnpm install:cli` 构建 CLI 及 workspace 依赖，将命令链接到 `~/.local/bin/oxitone`；
  可通过 `node scripts/install-cli.mjs <bin-directory>` 选择目录。不覆盖其他安装；
  bin 目录不在 PATH 时提示用户添加，而不是修改无关 shell 配置。
- viewer 以平台二进制分发：`@oxitone/preview-darwin-arm64`（x64 同规则），npm 安装，规则同 native 包；不允许运行时下载。
- GPUI 依赖 pin 到具体 git rev，升级必须过 viewer 的 smoke test；GPUI 的 UI 线程模型不得反向约束音频线程。

当前源码入口和 unsigned 开发 app bundle 已提供，`pnpm build:preview` 构建后
`pnpm preview <entry.ts>` 默认 watch；`--viewer` 可指定二进制或 `.app`，`--no-watch`
只构建一次。GPUI pin 为 `69e2130295c2649963eb639fc70b4f2ee8ea1624`，必须启用 `font-kit` 原生文字后端；关闭默认 features 时仅启用 runtime_shaders 会落入 dummy 文字系统，导致文字消失。启动 GUI 检查系统字体列表非空；headless 不初始化文字后端。runtime_shaders
不依赖单独的 Metal compiler。正式 npm 平台包与签名分发仍是发布门禁，不能把本地
bundle 视为已发布包。详细入口/controls 示例见 `docs/preview.md`。
开发 bundle 重建使用临时副本 + rename 原子替换 executable，保证新 inode；禁止原地覆盖已运行过的 Mach-O，避免 macOS 沿用旧代码签名缓存并以 SIGKILL 拒绝启动。

首版 piano roll 呈现选中 Pattern 的原始音符，按 native dispatch 回显 note gate；
scope true-peak 为 UI 消费音频的 4× 估计，丢帧时不能代替 export report。轨道和
mixer 状态来自 source，动态 meter 来自引擎。Loop selection 选择 clip 区间；时间线点击设置 cue。
Locate 输入与 G 快捷键已移除，cue 使用时间轴、marker 和导航快捷键。当前换图保留 transport 并重建
voice；启动 realtime session 后改变 sampleRate/blockSize 需重启。大工程虚拟列表、
手动主题覆盖/偏好持久化、未锁屏桌面的原生交互验收及真实设备 endurance 继续单独追踪。

## 测试

`node scripts/smoke-builtin-panels.mjs` 在临时工程逐个打开 30 个内置插件，使用 simulated sink。
`--light`/`--narrow` 检查主题和窄窗；`--only=wavetable --page=shaping|modulation|matrix` 检查合成器分页；
`--edit` 通过真实 NSEvent 验证旋钮命中、拖动投影、
共享 preset 的实例隔离、撤销/重做、Escape、Mix/bypass 与保存重开。
`--edit-synth` 验证波形选项命中、模式条件显示、波形拖动投影、撤销/重做、Escape、实例隔离与保存重开。截图与日志位于
`target/builtin-panels/`。release `benchmark_builtin_plots` 仅测 UI source 图形模型构建，
不代表 GPU 绘制、音频 callback 或 effective 遥测。

Mixer、Track/BPM、片段复制/长度/启停/删除和插件实例选择的当前交互与源码事务见
[23](23-daw-controls.md)。`OXITONE_PREVIEW_CAPTURE_CONTROLS=1` 验证控件的真实命中和
键盘通路，Save 后从全新工程进程校验结果；测试仍仅使用 simulated sink。
`OXITONE_PREVIEW_CAPTURE_EDITING=1` 的钢琴分支增加原生力度绘制、跨越末尾插入、
长度的 Undo/Redo、保存重开和力度区右键隔离。多声部的播放指针使用外层 Pattern 周期。

- `node scripts/smoke-ui.mjs` 串行运行两套主题、1440×920 与 1060×720 的真实窗口场景，
  PNG、日志及结果写入 `target/ui-review/`。包括钢琴/曲线编辑、内部窗口、插件配置及关闭保存，
  每次在临时工程编辑后重新打开验证源码。DAW 始终使用 simulated sink。
- `OXITONE_PREVIEW_CAPTURE_UI_REVIEW=1` 配合 `scripts/smoke-daw.mjs` 验证弹层输入隔离、
  插件 Edit configuration 的真实按钮命中、错误代码恢复、AppKit 关闭拦截、保存锁失败后保留草稿，
  最后保存关闭并重开核对。`OXITONE_PREVIEW_CAPTURE_OUTPUT` 可指定该脚本的 PNG 路径与相邻 `.log`。
  错误草稿中的 Add package/Enter 用捕获的控制队列验证安装请求和失败反馈，不调用 npm 或网络。
  定向 NSEvents 与 GPUI 键盘分发不替代物理鼠标、IME、VoiceOver、交通灯及原生全屏验收。
- runner↔viewer 的 IPC 帧用 protocol fixture 做兼容性测试（含 revision 乱序、诊断帧、大 snapshot）。
- viewer 的状态归约（snapshot → 视图模型）为纯函数，单元测试覆盖；UI 像素级测试不做。
- 两套主题检查正文/控件/诊断/琴键/clip 文字对比度 ≥ 4.5:1、scope/meter 信号 ≥ 3:1；
  原生拖动、交通灯、全屏和运行中外观切换仍需未锁屏桌面交互验收。
- 集成冒烟：示例工程启动 preview，断言 transport 命令生效、换图不中断、诊断 overlay 路径可达。
- viewer 关闭连接时，runner 在 socket end/close 后停止发送队列，并在异步清理前关闭队列；
  不向 writableEnded/readableEnded 的 socket 写最终 shutdown。半关闭回归测试不打开音频设备。
- `OXITONE_PREVIEW_CAPTURE=<PNG>` 启用开发截图：在 UI 线程额外驱动真实 NSView 重绘，工程就绪后调用系统窗口截图并退出；默认不启动播放、不改系统偏好。所有 capture 分支强制使用 simulated sink，包括内置插件面板，避免测试入口遗漏后打开系统音频设备。`OXITONE_PREVIEW_SIMULATED=1` 可供持续的 GUI 人工验收使用，同样不打开音频设备。仅截图模式允许 `OXITONE_PREVIEW_APPEARANCE=light|dark` 覆盖单个窗口，以及 `OXITONE_PREVIEW_CAPTURE_SIZE=1060x720` 指定逻辑窗口尺寸。
- `OXITONE_PREVIEW_CAPTURE_NAVIGATION=1` 在截图模式加入 GPUI 键盘分发、基于真实布局边界的滚轮/拖动控制器冒烟，验证钢琴缩放/滚动、Mixer 选择及滚动。它不等于物理鼠标/触控板与系统交通灯、拖动、全屏验收；后者仍需未锁屏桌面。
- `OXITONE_PREVIEW_CAPTURE_PLUGIN=instrument|synth|effect|info` 在截图模式打开音源和效果器详情，验证重复打开复用、分步关闭重开和键盘滚动；分别截图第一个音源、第一个 Wavetable、第一个 Channel effect 或其插件信息。关闭主窗口走 AppKit 的正常 should-close 路径，验证详情仍打开时 session 可以退出。`OXITONE_PREVIEW_CAPTURE_REVISION` 指定截图前必须接受的最低 revision（默认 1）。
- `node scripts/smoke-preview-details.mjs [viewer]` 使用真实鼓机/gain dylib，在详情窗口已打开后修改临时入口的 volume，断言所有窗口跟随 revision 2、音源显示新值并成功截图；不修改示例文件，不启动播放。需要先构建 workspace、鼓机示例动态库与 viewer。
- `node scripts/smoke-preview-panels.mjs [viewer]` 覆盖多窗口语法/runtime/native 失败、坏布局局部回退、Mix 与源码同步、UI import 更新和恢复。截图开关 `OXITONE_PREVIEW_CAPTURE_WATCH=1` 仅在插件截图模式输出观察状态并延长等待；`OXITONE_PREVIEW_CAPTURE_PAGE` 与 `OXITONE_PREVIEW_CAPTURE_PLUGIN_SIZE=440x400` 检查分页和窄窗口。
  此测试主动注入故障，期间出现的错误 UI 是验收内容；脚本逐阶段标记预期故障，成功后
  总结恢复结果。完整 stdout/stderr 写入 `target/*-panels-watch.log`，`--verbose` 可实时
  输出；真正失败仍输出原始日志并返回非零，不屏蔽产品诊断。
- `OXITONE_PREVIEW_CAPTURE_MIXER=split|expanded|chain` 搭配 `examples/drum-machine/src/mixer-preview.ts` 验证发送比例/自动化、发送目标与输入反向导航、右侧独立键盘滚动；与 navigation/plugin capture 模式分别运行。该可运行示例独立于原始歌曲导出，包含 Drum/Music bus、Reverb/Delay return、pre/post send 和 sidechain，不伪造 UI 数据。
- `OXITONE_PREVIEW_CAPTURE_TRANSPORT=1` 必须与其他 capture 冒烟分别运行。它使用真实 GPUI 键盘分发、基于实测布局的鼠标控制器和真实 Rust engine + simulated sink，验证时间线／钢琴点击、双击与 Alt 播放、cue 重播/停止、循环边界、连续定位与多窗口快捷键，断言 snapshot 不变。该模式会播放模拟输出，始终不打开音频设备；普通截图不启动播放。它不证明物理鼠标命中、设备 callback 或 xrun 长测结果。
