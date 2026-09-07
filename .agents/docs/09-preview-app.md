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

- **Workspace/arrangement**：Track 列表、PatternClip/SampleClip 在时间轴上的布局、markers、loop region；静态结构来自 snapshot。
- **Piano roll**：每个 Pattern 的 note 网格、播放头、当前发声 note 高亮（note-on/off 经 meter 通道回显）。频谱/波形见下。
- **Channel rack 与 Mixer**：Channel/MixerChannel 列表、level/pan/mute/solo 状态、peak/RMS meter、Master true-peak、send 路由的只读图示。
- **Scopes**：波形（各 bus 峰值 ring）、频谱（FFT）、相位空间 XY/vectorscope（M/S 分解）。
- **Transport 条**：play/pause/stop/seek（bar/beat/marker/点击时间轴）、loop region、当前 bar.beat.tick 与 timecode、tempo 显示（含 tempo lane 烘焙结果）。

## 分析数据路径（实时纪律）

- audio 线程只写预分配 ring/atomic：meter 值、降采样峰值、analysis 帧、transport 游标。**FFT、加窗、M/S 分解都在 viewer 的 UI/分析线程执行**，从 ring 取数。
- UI 以约 30 fps 节流消费；满 ring 丢新 analysis 帧并计数，transport ack 不允许丢（沿用引擎规则）。
- 播放头显示必须补偿 `getOutputLatency()`：画面位置 = engine 游标 − 输出延迟，使视觉与听觉对齐。
- viewer 显示 `engineLoad`、underrun 计数和插件 fault 归因，与 `05` 的诊断指标一致。

## 交互与延迟

- viewer 的 transport 操作走引擎同一 command queue；生效延迟 = ring horizon（见 `03`），UI 据此做预期反馈（按钮立即响应，播放头按 horizon 对齐）。
- seek 目标支持 bar/beat/marker/timecode 与时间轴点击；点击位置按当前有效 tempo map（含烘焙的 tempo lane）换算。

## 打包与启动

- `oxitone preview <entry.ts>`：CLI 启动 runner，并拉起/连接 viewer。
- viewer 以平台二进制分发：`@oxitone/preview-darwin-arm64`（x64 同规则），npm 安装，规则同 native 包；不允许运行时下载。
- GPUI 依赖 pin 到具体 git rev，升级必须过 viewer 的 smoke test；GPUI 的 UI 线程模型不得反向约束音频线程。

当前源码入口和 unsigned 开发 app bundle 已提供，`pnpm build:preview` 构建后
`pnpm preview <entry.ts>` 默认 watch；`--viewer` 可指定二进制或 `.app`，`--no-watch`
只构建一次。GPUI pin 为 `69e2130295c2649963eb639fc70b4f2ee8ea1624`，runtime_shaders
不依赖单独的 Metal compiler。正式 npm 平台包与签名分发仍是发布门禁，不能把本地
bundle 视为已发布包。详细入口/controls 示例见 `docs/preview.md`。

首版 piano roll 呈现选中 Pattern 的原始音符，按 native dispatch 回显 note gate；
scope true-peak 为 UI 消费音频的 4× 估计，丢帧时不能代替 export report。轨道和
mixer 状态来自 source，动态 meter 来自引擎。Loop selection 选择 clip 区间；Go
支持 bar.beat（均从 1 起）、mm:ss 或 `s` 后缀秒数。当前换图保留 transport 并重建
voice；启动 realtime session 后改变 sampleRate/blockSize 需重启。大工程虚拟列表、
主题持久化、锁屏后的视觉验收及真实设备 endurance 继续单独追踪。

## 测试

- runner↔viewer 的 IPC 帧用 protocol fixture 做兼容性测试（含 revision 乱序、诊断帧、大 snapshot）。
- viewer 的状态归约（snapshot → 视图模型）为纯函数，单元测试覆盖；UI 像素级测试不做。
- 集成冒烟：示例工程启动 preview，断言 transport 命令生效、换图不中断、诊断 overlay 路径可达。
