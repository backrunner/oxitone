# Oxitone Preview App（GPUI Viewer）

## 定位与边界

Preview app 是 Oxitone 工程的可视化呈现器：**代码是音乐的唯一事实来源，viewer 只读**。用户可以 play/pause/stop/seek/loop，可以调整 viewer 自身的显示偏好（缩放、主题、scope 开关），但任何操作都不会反向修改代码或 authoring 数据。viewer 内不提供任何编辑能力（无拖拽音符、无画 automation、无推子写回）。

与代码的同步是单向数据流：

```text
TS 代码变更 -> runner 重新执行 -> ProjectSnapshot(revision+1)
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
runner 用 esbuild 追踪本地静态 import 依赖（含解析失败的目录；npm packages external），以独立 Node/tsx 子进程
执行原始入口，避免模块缓存、保留 import.meta.url。每轮执行有 10 秒超时、64 MiB
结果上限；代码日志走 stderr，snapshot 单独走 pipe，不混用 stdout JSON。
运行期动态 import/文件读取依赖可由 `--watch-path <path>` 显式补充。未使用文件扫描
或远程下载来寻找插件。

IPC 是仅本机 Unix socket，目录 0700、socket 0600；4-byte big-endian 长度 + UTF-8
JSON，单帧最大 64 MiB。`preview-frame.schema.json` 定义 snapshot（含 revision）、
diagnostic、status、transport、query、shutdown；所有帧含 protocolVersion 1.0。
`preview-response.schema.json` 定义有序 state/rejected 应答；runner 一帧 in-flight，
队列保留各类型最新状态，避免慢编译积累大快照。native rejected 清除该 revision 的
去重缓存，下次 watch 事件允许重试相同内容。只有 snapshot 能替换 authoring 呈现，transport 仅改变播放状态。viewer 断线仍持有
最后合法图；runner 显式退出才发送 shutdown。`--headless` 使用模拟输出供集成测试。

- revision 单调递增；viewer 忽略乱序或重复 revision。
- snapshot 内容 hash 不变时不触发重编译（runner 负责 diff）。
- 编译/校验失败：保持当前可播放图（引擎既有语义），viewer 显示结构化诊断 overlay，包含稳定错误码和 JSON path。
- 换图在 block 边界完成；正在播放时换图不中断 transport，seek/loop 状态保留。

## 视图（全部只读）

- **Playlist/arrangement**：固定 Track 侧栏与小节标尺，PatternClip 的彩色标题与真实 note 缩略图；缩略图遵循 Track tempo、clip 重复和截断。首次打开适配工程长度，markers 独立导航。未命名 Pattern 按全局首次出现顺序编号，同一 Pattern 的多个 clip 共用名称，派生标签按 snapshot 缓存。
- **Piano roll**：完整 128 MIDI 音高范围，固定琴键、局部 Pattern 拍标尺和 velocity lane；自动适配选中音符，播放头及 native note-on/off 高亮。音符与琴键可滚动、缩放，不能编辑。
- **Mixer**：紧凑通道条、固定 Master、通道颜色、只读 level/pan/mute/solo、分段 peak/RMS 表（两列不是 L/R）；独立可收起的效果器/路由 inspector 展示完整名称、bypass、send 和 Master scope true-peak。
- **插件详情窗口**：Mixer 的音源入口、Channel/Mixer/Master 的每个效果器槽位均可独立打开。窗口按 `Instrument(channelId)`、`ChannelInsert(channelId,index)`、`BusInsert(busId,index)` 复用；不同实例/槽位可并排查看。当前 EffectRef 没有实例 ID，watch 时窗口跟随槽位索引，重排后显示该槽位的新插件；删除后显示未挂载状态，再出现时恢复。
- 详情来自已接受快照与控制线程复制的权威 descriptor，包含参数显式值/默认值、物理范围/单位、平滑/rate/mapping、自动化绑定、效果器 mix/bypass、音源资源/structured state、布局/复音/capabilities、内置或动态库来源及已验证 hash。参数是 source 初始配置，不能冒充自动化/平滑后的实时有效值；不创建第二个 DSP 实例、不调用插件 process/getter、不持有动态库或 DSP 指针。
- **Scopes**：波形（各 bus 峰值 ring）、频谱（FFT）、相位空间 XY/vectorscope（M/S 分解）。
- **Transport 条**：play/pause/stop/seek（bar/beat/marker/点击时间轴）、loop region、当前 bar.beat.tick 与 timecode、tempo 显示（含 tempo lane 烘焙结果）。

## 分析数据路径（实时纪律）

- audio 线程只写预分配 ring/atomic：meter 值、降采样峰值、analysis 帧、transport 游标。**FFT、加窗、M/S 分解都在 viewer 的 UI/分析线程执行**，从 ring 取数。
- UI 以约 30 fps 节流消费；满 ring 丢新 analysis 帧并计数，transport ack 不允许丢（沿用引擎规则）。
- 播放头显示必须补偿 `getOutputLatency()`：画面位置 = engine 游标 − 输出延迟，使视觉与听觉对齐。
- viewer 显示 `engineLoad`、underrun 计数和插件 fault 归因，与 `05` 的诊断指标一致。

## 交互与延迟

- 窗口内容延伸到顶部，系统标题文字/独立标题栏隐藏；自绘 56 px 标题区整合工程名、
  构建状态与只读标识，保留原生 macOS 交通灯按钮。非交互标题区可拖动，双击遵循
  系统标题栏偏好；Scopes 按钮独立于拖动区域。全屏时收回交通灯预留空间。
- 外观始终跟随当前窗口的系统 Light/Dark（含 Vibrant）appearance，启动时读取并
  订阅运行中的变化，无须重启。轨道、钢琴窗、Mixer、Scopes、诊断、按钮 hover/active
  和位置输入 focus 共用语义配色；主题不进入 ProjectSnapshot，不触发编译或音频命令。
- viewer 的 transport 操作走引擎同一 command queue；生效延迟 = ring horizon（见 `03`），UI 据此做预期反馈（按钮立即响应，播放头按 horizon 对齐）。
- seek 目标支持 bar/beat/marker/timecode 与时间轴点击；点击位置按当前有效 tempo map（含烘焙的 tempo lane）换算。
- Playlist 支持双轴滚动与拖动滚动条，Shift 滚轮横向、⌘/Ctrl 滚轮缩放，Fit 恢复全工程范围；长时间轴按可见范围创建标尺刻度。
- Piano 支持双轴滚动与拖动滚动条，Shift 滚轮横向、⌘/Ctrl 滚轮缩放时间，Keys ± 调整音高行高，Fit 恢复适配。点击后方向键、Page Up/Down、Home/End 移动视口，± 缩放、F 适配；点击局部拍标尺 seek，Loop clip 使用全局 clip 边界。
- Mixer 滚轮/触控板与 ‹/› 按钮横向浏览；面板过矮时 Alt 滚轮或纵向滚动条浏览下部；Master 同步纵向位置。点击后左右/Home/End 选择并显示通道，上下/Page Up/Down 纵向浏览；inspector 独立纵向滚动。选择只改变分析 scope。
- Playlist 与下部编辑视图之间、Piano 与 Mixer 之间的分隔线可拖动。Space 播放/暂停，位置输入框独立处理文字。所有视口状态仅在 viewer 内持有，watch 换图保留仍有效的选择并按新边界夹紧视口。
- 详情窗口沿用自绘标题区、原生交通灯与系统 appearance；支持参数筛选、滚动、参数/资源/插件信息切换和复制该引用的 JSON。重复打开聚焦已有窗口；独立关闭不影响播放或其他窗口，关闭主窗口退出整个配对 session。有效 watch revision 才更新详情，失败编译保持上次合法数据。dylib 自定义 UI 的后续方案见 `11-plugin-ui.md`，当前通用详情不执行插件 UI 代码。

## 打包与启动

- `oxitone preview <entry.ts>`：CLI 启动 runner，并拉起/连接 viewer。
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
mixer 状态来自 source，动态 meter 来自引擎。Loop selection 选择 clip 区间；Go
支持 bar.beat（均从 1 起）、mm:ss 或 `s` 后缀秒数。当前换图保留 transport 并重建
voice；启动 realtime session 后改变 sampleRate/blockSize 需重启。大工程虚拟列表、
手动主题覆盖/偏好持久化、未锁屏桌面的原生交互验收及真实设备 endurance 继续单独追踪。

## 测试

- runner↔viewer 的 IPC 帧用 protocol fixture 做兼容性测试（含 revision 乱序、诊断帧、大 snapshot）。
- viewer 的状态归约（snapshot → 视图模型）为纯函数，单元测试覆盖；UI 像素级测试不做。
- 两套主题检查正文/控件/诊断/琴键/clip 文字对比度 ≥ 4.5:1、scope/meter 信号 ≥ 3:1；
  原生拖动、交通灯、全屏和运行中外观切换仍需未锁屏桌面交互验收。
- 集成冒烟：示例工程启动 preview，断言 transport 命令生效、换图不中断、诊断 overlay 路径可达。
- `OXITONE_PREVIEW_CAPTURE=<PNG>` 启用开发截图：在 UI 线程额外驱动真实 NSView 重绘，工程就绪后调用系统窗口截图并退出；不启动播放、不改系统偏好。仅此模式允许 `OXITONE_PREVIEW_APPEARANCE=light|dark` 覆盖单个窗口，以及 `OXITONE_PREVIEW_CAPTURE_SIZE=1060x720` 指定逻辑窗口尺寸。
- `OXITONE_PREVIEW_CAPTURE_NAVIGATION=1` 在截图模式加入 GPUI 键盘分发、基于真实布局边界的滚轮/拖动控制器冒烟，验证钢琴缩放/滚动、Mixer 选择及滚动。它不等于物理鼠标/触控板与系统交通灯、拖动、全屏验收；后者仍需未锁屏桌面。
- `OXITONE_PREVIEW_CAPTURE_PLUGIN=instrument|synth|effect|info` 在截图模式打开音源和效果器详情，验证重复打开复用、分步关闭重开和键盘滚动；分别截图第一个音源、第一个 Wavetable、第一个 Channel effect 或其插件信息。关闭主窗口走 AppKit 的正常 should-close 路径，验证详情仍打开时 session 可以退出。`OXITONE_PREVIEW_CAPTURE_REVISION` 指定截图前必须接受的最低 revision（默认 1）。
- `node scripts/smoke-preview-details.mjs [viewer]` 使用真实鼓机/gain dylib，在详情窗口已打开后修改临时入口的 volume，断言所有窗口跟随 revision 2、音源显示新值并成功截图；不修改示例文件，不启动播放。需要先构建 workspace、鼓机示例动态库与 viewer。
