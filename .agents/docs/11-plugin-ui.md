# Plugin UI：通用详情、声明式界面与原生扩展

状态：P0 通用详情与 P1 声明式 GPUI 原生面板已实现；P2 独立 NSView companion 和 P3 effective 遥测仍为规划。
本方案补充 `08-plugin-abi.md` 与 `09-preview-app.md`，不改变现有 audio ABI v1。

## 已交付的基础层（P0）

- 每个音源和 Channel/Mixer/Master insert 都可打开只读 GPUI 详情窗口；相同地址聚焦已有窗口，多个地址可同时打开。
- 控制线程从已校验 registry 复制 descriptor 与动态库路径/实际 SHA-256，详情不创建 DSP 实例，不持有库或音频实例指针。
- 显示源码初始参数、缺省参数、范围/单位/平滑/rate/mapping、自动化绑定、mix/bypass、音源资源和 structured state；用户可以筛选、滚动、切换标签及复制引用 JSON。
- 有效 watch 快照更新所有窗口；拒绝的构建不改变窗口数据。窗口键是 owner kind + owner ID + instrument/insert index。EffectRef 无独立 ID，重排后窗口跟随槽位，不能用 pluginId 作为实例 ID；同插件的多个实例必须互不混淆。
- 关闭详情仅销毁该窗口；关闭主窗口退出 session。详情跟随系统 appearance，使用自绘标题区和原生交通灯。

## 选择：先提供由 GPUI 渲染的声明式 UI，再提供可选原生 UI

| 形式 | 提供方 | 优点 | 代价 / 适用范围 |
| --- | --- | --- | --- |
| 通用详情（当前） | 宿主按 descriptor 生成 | 所有内置与 dylib 插件立即可查看 | 不表达插件自己的视觉结构 |
| 声明式 UI（P1，已实现） | npm 插件包导出版本化 TS/JSON 布局 | 自定义布局/标题、统一主题和只读约束；不执行第三方 UI 代码 | 受宿主向量组件集合约束 |
| 原生 UI（P2，可选） | 插件包附带 UI companion dylib | 插件可提供自己的 AppKit/Metal 视图 | 平台专用；同进程原生代码故障可能拖垮整个 viewer |

GPUI 是宿主窗口和声明式组件的渲染层，不把 GPUI/Rust trait 或 Entity 通过 dylib ABI 暴露。
不要求插件作者与宿主使用同一 Rust 编译器或 GPUI git revision，也不要求提供浏览器/JavaScript UI。

## P1：已实现的独立 UI 注册与布局协议

`Project.registerPluginUi(layout: PluginUiManifest): this` 为精确的 pluginId/pluginVersion 注册 GPUI 原生面板，
支持内置与 dylib 插件；同一身份再次注册替换之前的布局。输入和 `registeredPluginUis` getter 均复制，
不修改音乐 revision，不写入 ProjectSnapshot/便携工程/DSP manifest，不创建 DSP 实例。
布局对象可随 npm 包导出，在工程入口显式注册。静态 TS/JSON import 自动 watch；external npm
包或运行期文件读取需要 `--watch-path`。UI schema 是 `schemas/plugin-ui.schema.json`。

```ts
project.registerPluginUi({
  uiVersion: "1.0", pluginId: "oxitone.delay", pluginVersion: "1.0.0",
  title: "Echo", size: { width: 520, height: 320 },
  pages: [{ id: "main", title: "Delay", groups: [{
    id: "echo", title: "Echo", columns: 3, controls: [
      { kind: "knob", parameter: "timeBeats", label: "Time" },
      { kind: "knob", parameter: "feedback", label: "Feedback" },
    ],
  }] }],
});
```

- 固定三层结构 pages → groups → controls；groups 自动换行，columns 指定组内列数，窄窗口减少列数。
  支持 knob、水平 fader、toggle、readout、choice、ADSR envelope 及下述 source 可视化；页面切换/滚动/窗口缩放是显示操作。
  size 指定初始逻辑尺寸；有效 watch 改变 size 时调整非全屏窗口，其余更新保留用户尺寸。
- 所有控件绑定 **plugin namespace** 中的 descriptor ID；UI 无法覆盖范围/单位/默认值/mapping。
  choice 绑定 enum，值必须有限、整数、唯一且在 descriptor 范围内；未列出的值显示原始值。
  toggle 仅绑定 0/1 enum。ADSR 的 A/D/R 必须是非负 seconds，S 范围在 0…1；
  图为 source 参数的折线示意，固定示意 sustain hold，不宣称复制 DSP 包络曲线。
- 每个 Channel/Bus/Master effect 的 **host namespace** mix/bypass 由宿主固定绘制，不接受布局覆盖，
  在所有标签和滚动位置都可见。Mix 是 0…1 wet 比例，缺省为 1；显示百分比和 dry/wet，自动化有标记。
  同名插件参数与 host Mix 分开解析。参数从工程代码设置，Preview 无参数 setter。
- 控件为 GPUI 原生向量绘制，跟随 light/dark 语义主题；无 HTML、脚本、下载、图片或任意原生视图入口。
  品牌标题和控件标签可自定义；额外组件、图片资源/字体/自定义着色器留待后续协议扩展。
- 限制：最多 64 registrations / 总计 2 MiB；每布局 256 KiB、8 pages、每页 16 groups、
  每组 32 controls、整布局 256 controls，columns 1…6，标签 64 UTF-16 单位、ID 128。
  禁控制字符、重复 page/group ID、重复注册和未知字段；逻辑 width 440…1200、height 280…900。
  Wire 的可选 `pluginUis` 保留未知 JSON，由 viewer 在控制线程独立校验，避免布局错误拒绝合法音乐。
- 错误码 `PluginUiInvalid` 属于局部 UI 诊断。未知组件/参数、超预算、不兼容版本时，保留相同身份且
  对当前 descriptor 仍合法的最后面板，否则用紧凑自动面板；成功音频图仍可接受。移除合法注册恢复默认面板。
  缺身份/非法注册容器保留兼容旧布局并显示诊断；不同插件身份绝不复用旧布局。
- 控件消费 source/default；自动化用小标记区分，effective/live 读数尚未提供。Inspect 随时访问全部参数，
  包括自定义页面未展示的参数；Assets/Info 保存资源、state、descriptor 和已验证 dylib path/hash。
- Wavetable 默认具备 Oscillators/Modulation 页面，涵盖双振荡器波形渐变、
  滤波响应、Voice/Output/Sub/Noise、两个 ADSR、LFO source 曲线及固定路由深度；
  example.drums、fixture.gain 在鼓机示例通过公开 TS API 注册自己的面板。其他插件按 descriptor 紧凑分组。
  Modulation 把两个 ADSR 放在 LFO/路由下方，减少切页和固定窗口内的空白。

新增 source visual controls 同样可由第三方 `registerPluginUi` 布局使用，uiVersion 仍为 1.0：

| kind | 绑定与图形语义 |
| --- | --- |
| `oscillator` | wave/morphTo 对应 0…5 的六种内置 cycle；position/phase/spread 是 0…1，unison 为 1…16 enum，detune 为 0…100 cents。可选 bank/warpMode（0…3 enum）、warp（0…1）、octave（−4…4 enum）绑定。UI 线程读取共享准备表生成周期图，显示 source/bank 插值及 warp、octave；2D/3D 仅切换堆叠曲线 |
| `subOscillator` | wave 为 sine/triangle/saw/square/pulse/rounded（0…5 enum），octave −4…4 enum，level 0…1；显示独立 Sub 周期、octave 和幅度 |
| `filterResponse` | mode 对应 LP/HP/BP enum 0…2，cutoff 为正 Hz，resonance 为 0…1。用项目 sampleRate 和 Q=0.5+9.5r 的 biquad 系数计算对数频率响应 |
| `lfoCurve` | shape 对应 sine/triangle/ramp/square enum 0…3，rate 为正 Hz，phase 为 0…1，显示一个 source 周期及周期秒数 |
| `modulation` | routes 为 1…8 个 `{label, amount}`，amount 绑定现有参数，展示带物理单位的路由深度与双极条 |

以上组件不是任意第三方算法的自动分析器；插件使用它们即声明相同 cycle/filter/LFO
含义。单位/范围/ID 不兼容时走 `PluginUiInvalid` fallback。旧宿主遇到新 kind
同样局部回退，不拒绝音乐。绘图在 UI 线程复用解析式 source cycle/LFO 与 DSP
biquad design，不创建音频实例。波形堆叠不代表 unison 当前 phase，滤波曲线不含
filter envelope/LFO 的瞬时作用；不宣称 effective/live plugin telemetry。

### 同步与工程修改边界

runner 每次新 worker 序列化独立 `pluginUis` 元数据，内容 hash 包括 UI，音乐快照不变。
viewer 自行计算 snapshot（revision 清零）+ assetBaseDir + DSP registrations/hash + policy 的
源内容摘要，不能信任调用者的 hash 跳过验证。同样的音乐源只发布新的 ViewProject，复用 graph、
telemetry、plugin catalog、transport 及音频实例，不 reset 音源/效果器或清空分析历史。
音乐/资源引用/注册内容变化仍走完整 compile 和原子换图。资源与库的实际更新继续要求显式重建、
校验 hash 并重新 authoring；UI 变更不作为重新加载音频资源的隐式请求。

只有 native 接受的快照和局部处理后的布局一起发布；所有窗口观察同一个已接受 Arc。
building、语法错误、runtime exception、执行超时、缺失依赖、native 编译失败时，各窗口显示
Building/Last good 状态，保留最近有效参数、Mix 和布局；恢复后同时更新。页面按稳定 ID 保留，
失效页面回到首个页面；效果器窗口仍跟随 owner + slot index。关闭窗口不影响音频生命周期。

测试包含共享 TS→JSON→Rust fixture、参数绑定/预算/错误回退、UI-only graph/telemetry 复用、
同 hash 不绕过音乐校验、Mix 更新；实际多窗口 watch 冒烟覆盖语法/runtime/native 拒绝、
坏布局保留上次界面和后续恢复。release 布局基准测控制线程 JSON 解析与校验，不能代表 GPUI
绘制或音频 callback 性能。未锁屏物理输入与独立 NSView companion 仍不由截图替代。

## P2：独立版本的原生 UI C ABI（macOS-first）

建议入口 `oxitone_plugin_ui_entry_v1`，UI ABI 与 audio ABI 独立协商；该 symbol 目前不加载、不调用。
优先独立 `lib<plugin>_ui.dylib`，DSP dylib 的 audio entry 不变。宿主按显式注册路径/hash/签名策略验证 UI library，平台包与 DSP 包一起分发，不能静默放宽 `signed-only`。

每个入口/host 表都以 `abi_major, abi_minor, struct_size` 开头，仅传固定宽度整数、计数字节切片、C 函数指针及 opaque UI handle。初始接口职责：

| 操作 | 所有权与线程 |
| --- | --- |
| query capabilities / size | 主 UI 线程；协商 `macos-nsview`、只读模式、主题和尺寸支持；无 DSP 实例 |
| create | 主 UI 线程；独立 UI 实例接收复制的插件身份、slot key、graph generation 和只读上下文 |
| attach | 宿主创建窗口及原生容器 NSView，插件附加自己的 NSView；parent 为借用对象，不得销毁宿主窗口 |
| update snapshot | 主 UI 线程；版本化、带 revision/generation 的有界 source/default 数据；调用期借用，插件要持久保存则复制 |
| resize / scale / appearance / visibility | 宿主传逻辑 points、backing scale、语义主题及可见性；插件不能同步进入嵌套窗口事件循环 |
| detach / destroy | 主 UI 线程；先移除子视图、取消回调/异步任务，再销毁 UI handle；最后释放该 UI library 引用 |

GPUI 继续拥有窗口、自绘标题区和交通灯。原生内容在独立的 AppKit 容器中承载，不把外部 NSView 误当 GPUI element；输入焦点、IME、Tab、Escape/⌘W、缩放和窗口关闭须有原型验收。

UI host 表不提供音频实例指针、render graph 指针、process/reset 调用或任意参数 setter。
通用能力只有按已声明 ID 订阅快照、请求合法显示尺寸和日志/诊断（非实时线程）。UI 写请求在 Preview 一律拒绝；未来若有可编辑产品，必须另行定义代码/authoring 的回写契约，不能绕过“代码为事实来源”。

graph swap 后 slot key 仍是显示地址，generation 必须更新；旧 generation 的回调/数据丢弃。
UI library 活期与 DSP library 活期相互独立，禁止因窗口关闭释放仍在播放的 DSP；也禁止为显示而创建第二个 DSP 实例。UI 关闭不停止 transport，主窗口退出必须回收全部 UI。

版本不兼容、缺失 symbol、初始化失败或 UI 资源错误：显示局部诊断并回退通用详情，保持合法音频图。
同进程 native UI 的野指针、阻塞和崩溃无法靠 C ABI 验证隔离；P2 不能宣称具备崩溃隔离或超时抢占。若发布要求第三方 UI 故障不影响音频，应另设 UI helper 进程与 IPC/surface 协议门禁。

## P3：可选实时参数/专属可视化反馈

- 先定义版本化 `PluginUiSnapshot`，每包带 engine/session、slot、graph generation、audible frame、序号和 drops。只有同代数据能进入当前窗口。
- effective 参数必须来自 Rust 引擎实际合并 host/automation、mapping 后的值；若要承诺平滑后数值，插件必须明确提供对应遥测，不能在 UI 用 source AST 猜测。
- prepare 期建立固定容量 ring/atomic 和订阅表；worker/audio 只写有界数值，无 allocation、锁、日志、JSON、N-API 或 UI 函数调用。控制/UI 线程节流读取并组装快照；满队列丢新可视化帧并计数。
- meter、频谱、包络、波表/鼓机状态使用可协商数据类型与预算。FFT/绘制在 UI/分析线程；窗口最小化或关闭后停止非必要采集。
- 图替换、参数表变化与插件故障使对应反馈失效，不能显示旧值为当前值。

## 分阶段出口

1. **P1**：TS/Rust/schema round-trip；未知 ID/预算/未知组件/旧宿主 fallback；内置 Wavetable 与真实 example.drums、fixture.gain 的声明式面板；双主题、多窗口、watch、只读约束和布局基准。
2. **P2**：公开 UI C header + C/Rust companion fixtures；动态库真实加载、ABI prefix/minor、主题/DPI/resize/焦点/IME、关闭再开、换图/移除、先销毁视图后卸载库；失败不换音频图；未锁屏 AppKit 实机验收。
3. **P3**：frame/generation/乱序/drops 测试，source/default/effective 区分；关闭窗口停止订阅；音频 PCM parity、allocation/free=0、开启多个 UI 的 worker 和真实 callback p95/p99/xrun 长测。

当前交付 P0/P1 的向量组件范围；P2/P3、图片资源扩展、第三方 UI 隔离及正式签名分发仍未完成。
