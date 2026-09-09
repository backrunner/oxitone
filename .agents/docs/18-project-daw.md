# Project Document、GPUI 编辑和插件目录

本增量在 [17](17-project-source-session.md) 的选定 Pattern adapter 之上新增完整工程求值与
GPUI 音符编辑通路。它是 [交付矩阵](../designs/source-daw/06-delivery.md) 的部分实现，
不是全部 P0–P5 完成声明。Engine snapshot 支持 1.2，Preview 外壳兼容 1.0，插件仍为 ABI 1；
Document control 独立协商 2.0，不将旧引擎数据重新标成 2.0。

## 启动与所有权

`oxitone daw song.ts [--no-watch] [--viewer path]` 打开真实工程，Node `ProjectDocument`
拥有全部登记 TS 缓冲区、revision、accepted revision、跨文件 Undo/Redo、Save 和冲突。
默认 root 为入口目录；API 的 sourceRoots 显式登记其他本地源码根。跳过 node_modules、
生成目录、符号链接目录、未登记的嵌套 package。现有文件上限 4096，每份文本 8 MiB、
全部文本 32 MiB，Undo 最多 128 个版本且总文本 32 MiB。源码发现和磁盘同步在读取阶段
按 byte budget 分块读取，不先用无界 `readFile`/`Promise.all` 分配超限文件。超预算先拒绝，保持原草稿。

每轮通过 esbuild 内存 overlay 及全新 Node 子进程执行工程 sync/async factory 一次。
AST 仅在完整值边界临时捕获，保留 directive prologue、callee/receiver/this/await；
已知 Pattern 和已放置 PatternClip 通过对象身份关联。捕获 key/handle 不写用户文件。
捕获函数由临时虚拟模块导入，模块自身的词法作用域读取运行时 hook；作者的 `globalThis`
或同名局部变量不遮蔽 hook。别名避开文件内全部标识符。选定 Pattern adapter 复用此机制。
新增 import 放在完整 directive 之后，保留其同行注释且不越过同行可执行语句；保留 shebang/
前置注释与 CRLF。虚拟模块及导入只存在于求值产物，不进入保存文本。
候选通过 snapshot 验证、原生控制侧 compile、模块/lock/显式资产读集复核后接受。
未保存代码立即出现在 View；语法/执行/compile 失败保留草稿及上一张合法图。
独立临时 engine 验证不表示播放引擎已经采用，GPUI 另核对原生 snapshot revision。
GPUI 的 Code/CreateFile 请求在非 building/saving/closed 状态可提交，使错误草稿仍能修复；
插件安装输入遵循同一操作就绪判定，工程因缺包而 invalid 时也可安装，不要求先得到合法音乐图。
音乐编辑继续要求 accepted revision 与原生投影一致。源码诊断与 native 诊断隔离，
未保存主窗口关闭保护与交互验收见 [09](09-preview-app.md)。

Save 复用 SourceSaveStore 多文件 fsync journal、恢复、所有权和第三方 hash 检查；`createFile`
允许在已登记根目录内创建 `.ts`/`.mts`，父目录须已存在；相对路径以 projectRoot 解析，
拒绝已有路径、声明文件、依赖目录、未登记的嵌套 package、链接别名和超限输入。
预算与代次检查在登记所有权前执行；并发新建不会覆盖更新后的文档。
新模块的相对 import/re-export 可在 Save 前读取内存文本，`.js`→`.ts`、`.mjs`→`.mts`
与保存后的解析优先级一致，extensionless 的 `.ts`/`index.ts` 遵循 esbuild 顺序；不扩展
普通 Node/build 不支持的 extensionless `.mts`。未落盘模块记录不存在的读证据，外部创建使其失效。
完整 tsconfig paths/package imports/条件导出等虚拟文件解析仍是后续门禁。
VS Code 提供 Create TypeScript Source File 命令，使用服务给出的 projectRoot，创建后自动打开 linked buffer。
创建、Undo/Redo 与已有文件共用历史；撤销已保存的新文件后，Save 发布删除，Redo 可再次创建。
journal version 2 用 null 明确区分不存在和已有空文件，支持创建/修改/删除的 pre/postimage；
先恢复日志再发现、读取和执行工程；只接受 journal 2，其他版本拒绝并保留恢复文件。新文件通过独占临时文件
和无覆盖 hard-link 发布；日志记录临时名，恢复校验 inode 后清理 link/unlink 间强杀留下的别名。
外部第三种镜像不覆盖；整文件删除/rename 的通用编辑器入口、新目录和资产创建仍未开放。
保存使旧监听任务失效，避免异步旧读结果回退已提交的 disk baseline。Save 不改变源码
revision。所有权禁止写 npm 包、pnpm store、未登记链接依赖；没有隐藏音乐主文件。

## 编辑范围和拆散

GPUI Piano 使用 Draw 单击插音并记住上次时值；松手前拖动调整落点和音高，Shift 绘制
拉出时值。Alt 从按下开始即可临时自由吸附，编辑模式不再将 Alt 插音变成播放命令。
Paint（B）连续绘制，按鼠标轨迹穿越的时间/音高网格补全快速横向、纵向和斜向笔画；
用 MIDI tick/音高去重，最多 4096 个新音符/手势。右键拖动擦除。
Select（E）/⌘拖动框选，⌘单击切换选中，⌘A 全选；选择组支持时间/音高拖动、右边缘
时值、velocity、Shift 拖动复制、⌘D 在选择末尾复制、Delete 删除、Q 起点量化。
方向键移动选择，Shift 上下移八度、Shift 左右改时值。吸附菜单直接选择 4/1/½/¼/⅛ beat 或
Free（1/960 beat），默认 ¼；工具栏磁铁按钮控制是否吸附，关闭磁铁或拖动期间按 Alt
使用无级 1/960 beat 偏移。组移动夹紧时间/音域时保留
相对间距。⌘滚轮以指针为中心缩放。Draw/Paint/Erase 的完整手势松手后只提交一条事务，
音符起点保持在显式 Pattern 长度内；复制超出范围时提示扩展源码长度，不偷偷改变其他
placements 的循环长度。音符尾部继续遵循既有截断语义。
中键或 ⌘/Ctrl+Alt 拖动平移，⌘/Ctrl+Shift 滚轮以指针为中心缩放音高行高。
音符移动、时值边缘、velocity 与平移显示对应光标；框选 overlay 限制在音符网格内。
Escape 取消；手势期间只改变本地预览，只有相关 response 结束 pending，接受后重新
关联选择。来源版本变化取消活动手势。界面默认 This clip，可显式选择 Shared definition。

定义范围的候选必须精确改变该 Pattern 的全部 placements。当前片段范围须有恰好一次
执行的本地引用、包围该引用的已捕获 placement，以及全工程等价检查；其他 clip、Track、
插件/资源/UI/路由/automation 和未使用 Pattern 不能顺带改变。未知映射或多次执行的
helper/循环边界拒绝，不用源码出现次序猜测实例对应关系。

稀疏编辑保留 chord/arp/repeat 等生成链；多次操作归约 `.edit`，已拆散 literal 直接更新。
`planMaterialize` 求值和校验具体候选，不改变缓冲区、revision 或磁盘。View 中 review 带
planId、before/after TS、修改文件、片段集合、前后音符数和失去生成关系的说明。
GPUI 显示当前/候选源码；用户确认后 `confirmMaterialize` 检查准确 planId、revision 及
候选读集再提交。Cancel、代码编辑、Undo、Save、外部变化使旧候选失效。事件不等于确认。

bound identifier 直接替换为 `new Pattern({ ...notes })`，import planner 复用可见运行时
import，避开 type-only、别名和遮蔽，保留其他 import/export。工厂调用/getter/await 则输出
`((originalExpression), new Pattern({ ...notes }))`：原表达式仍执行一次以保留副作用，
仅替换音乐返回值。UI 说明此行为；不能声称已经移除依赖执行或可以卸载该包。
后续 literal edit 保留这个前缀，不累积 wrapper。不透明 DSP 不能当音符拆散。
选中模块仅导入 npm helper、没有 SDK import 时，planner 从该文件解析已安装的 `oxitone`
或 `@oxitone/core` 决定新增 import，避免给仅安装 core 的项目引入缺失的 umbrella 包。
完整候选仍须通过真实模块求值与依赖读集复核；这不是完整条件导出/definition resolver。

现有引擎按旧事件身份随机，note chance 或所编辑 placement probability 非 1 时拒绝图形
音符编辑。source slice 拆散也拒绝，等待保持相位的窗口/runtime origin 迁移。

## 文档 IPC、外部编辑器和冲突

DocumentRequest 含 documentProtocolVersion/sessionId/requestId/baseRevision 和语义 operation。
同 requestId、同内容幂等返回；异内容拒绝。保留 256 个结果，内容指纹为 SHA-256，
不额外保留完整 code 请求文本。生产客户端使用 `stream/<client>/<sequence>`：client 为
1..64 个 ASCII 字母、数字、下划线或连字符，sequence 为无前导零的正 safe integer。
每个 Document 最多 64 个 stream，记录各自最大已接收 sequence；结果过期后旧序号拒绝，
新序号可继续，不因累计请求次数耗尽服务。客户端对同一 stream 有序提交，重连保留序号。
只接受该 stream 格式，任意旧式 ID 与非法序号均拒绝，不再维护 retired ID 兼容集合。
服务重启用 sessionId 隔离。
提交队列 64，单帧 64 MiB，超预算显式失败。DocumentEvent 与 correlated response 独立。

GPUI 反向队列随 Node 每 50 ms query 的 native response 返回；本地 transport/query 不
消耗它。Node 保留所有事务应答，只合并呈现/query。旧 Viewer 没有 Document 2.0 声明时
DAW 连接拒绝。这是有界轮询控制通路，不是新的 native engine duplex 协议。

启动打印独立 Unix editor socket，目录 0700、socket 0600，4-byte BE 长度 + JSON。
连接立即收到当前 View，发送 `code` 同步未保存文本，和 GPUI 共用 dispatcher/Undo/Save。
VS Code 扩展与自动重连客户端见 [21](21-editor-session.md)，未保存同步和 journal Save
仅针对扩展打开的 linked TypeScript buffers；普通文件 tab 仍通过 disk watch 同步。

file watch 使用父目录、文件过滤及 120 ms 防抖。不同文件外部修改可合并；同文件有不同
dirty 文本时保留 baseline/draft/disk，在 View 公开 conflicts。改代码/Undo 不消除冲突。
GPUI Use disk / Keep draft 发送显示版本的 diskHash，磁盘再次变化时拒绝并要求刷新。
解决后清空旧整文件历史；Keep draft 仍需 Save。新增 Merge 选项执行保守逐行三方合并：互不重叠
的 hunk 自动合并，重叠修改继续保留冲突并要求用户选择或手工编辑；语义级合并仍未实现。

## 插件目录和验证

GPUI Plugins 用紧凑列表提供全部 20 个内置插件和已发现外部版本；主视图只有名称、
音源/效果器类型、All/Instruments/Effects 筛选和搜索，不显示参数参考表、统计徽章或卡片。
Used in project 按需显示已接受工程的使用位置与编辑/面板入口；Details 按需显示包版本、
插件版本、许可、平台诊断、验证结果及包管理操作。重复名称附 vendor/version 以区分。
未使用插件也可见；浏览不创建 DSP，不执行 npm JS 入口，不加载外部库。

项目 package.json 的直接 dependencies/devDependencies/optionalDependencies 按正常
node_modules 祖先路径查找。已安装包的 `oxitone.plugins` 指向包内静态 JSON，遵循
plugin-install-manifest.schema.json：formatVersion 1、plugins 数组；每项包含 displayName、
vendor、可选 license、完整现行 PluginManifest、按 `darwin-arm64` 等键索引的 platforms，
平台项包含相对 library 路径和 sha256。包版本与插件版本独立，同包可声明多个插件。

metadata format 和 ABI major 分别验证，本批 manifest 仍使用 ABI 1 字段。路径限制在
所选包真实目录；跨包平台入口待实现。单 metadata 1 MiB、单包最多 256 插件、目录最多
4096 条。坏 metadata、缺库、不支持平台单独显示，不自动换内置插件。

Verify library 仅在独立可终止 Node helper 加载所选库，10 秒超时、输出预算及原生
hash/manifest/ABI/工程签名策略验证，无 DSP 实例/设备。未指定策略时 signed-only，
不会自动降级 any。校验前后复核 metadata 和库字节，refresh 保守重置验证状态。
验证不是 DSP prepare/实时行为认证，helper 也不是恶意原生代码沙箱。

独立实例配置窗口提供初始参数数值输入、增减/默认值、效果器 mix/bypass 和局部/共享配置范围，
以及串联 chain 拆散 review；源码派生、真实 C 插件与 npm 边界见
[19](19-plugin-configuration-source.md)。普通参数编辑与本地化都走 Document 事务。
实例与 typed automation、GPUI 重排及源码保存见 [20](20-plugin-instances.md)。
Document control 已增加无 shell 的 install/upgrade/uninstall/repair package tasks：包名和版本在 Node
侧校验，任务使用受控 package-manager argv、120 秒 timeout，完成后重新求值并刷新目录；失败时恢复
package.json 与 lockfile 快照，避免声明半升级；
失败返回 PluginInstallFailed/PluginTaskConflict，不能绕过 Document Service。GPUI manager
在 Details 对已发现 package 显示 Repair、Install、Update、Uninstall 操作。该任务会修改项目 manifest/
lock/node_modules，这是显式插件生命周期操作；音乐拆散仍不会调用它或改 node_modules。
VS Code 另提供 Install Plugin Package 命令，可输入尚未安装的 npm 包和版本；请求仍经过
Document Service 的包名校验、任务串行化和 manifest/lockfile 失败恢复。
GPUI Plugins 顶部提供 Add package，进入包名输入模式；Enter 安装、Escape 取消，支持 scoped
包与显式版本。包名输入和目录搜索分别保留；无效格式、工程未 ready 或请求忙碌时保留输入
并显示诊断，提交仍经过 `installPlugin`，不会直接执行命令行。
插件实例添加/替换/删除、外部注册回写、Mixer/tempo 和片段基础编辑已在 [23](23-daw-controls.md)
补齐。安装/升级/卸载的完整失败回滚、可靠的逐包修复、typed runtime commands、资源创建向导、
并行 rack/宏绑定迁移、ABI 2 resources/state/UI companion 尚未交付。目录与验证入口不能
作为“完整插件管理器已完成”的证据。

## Automation source 编辑

### Pattern 声部与 Playlist Automation

Engine 1.2 新增 `PatternSpec.parts: { channelId, patternId }[]`。一个 composite Pattern
包含 1–256 个不同 Channel 的声部，每个声部引用独立的 leaf Pattern，root 的 notes 为空，
声部长度不超过 root，不允许嵌套 composite。构造时 root 长度采用 lengthBeats 与全部
leaf 长度的最大值；延长一个声部自动延长容器，其他声部对象／音符不变。snapshot 恢复
不修正非法短 root，而是拒绝。构造时登记全部 leaf 并验证跨工程 Channel、
重复 ID、重复 Channel、缺失引用和长度。leaf 的原有生成链、notes、排序与来源保持独立。
例如 `new Pattern({ lengthBeats: 4, parts: [{ channelId: keys.id, pattern: melody },
{ channelId: bass.id, pattern: bassline }] })`。leaf 的 edit/transpose 等操作继续适用；
对 composite 直接调用音符变换会拒绝，要求明确选择声部，避免丢失路由。

Track 接受重叠的 Pattern/Sample/Automation placements。带 parts 的 Pattern 根据自身声部
路由到 Channel，与 Track.channelIds 无关；无 parts 的旧 Pattern 保留 Track 路由。
循环使用 root 周期，较短声部在剩余时间不触发新音符，原有 duration 长尾仍遵循 clip
结束边界；音符起点必须在所属 leaf 长度内。截断、transpose、velocity、Track tempo
同时作用于各个独立声部；Channel 继续绑定
Mixer。MIDI 按 Track 合并各声部的音符，保留既有 Track MIDI channel 语义。
钢琴窗增加 Channel 声部选择；编辑选中的 leaf 定义会更新全部引用该 leaf 的 Pattern placements，
界面标明 Pattern part。候选等价比较递归解析 parts，拒绝改变其他声部或编排位置。

`AutomationLane.playback` 为 global（省略时兼容旧数据）或 playlist。
首次 `Project.arrange({ kind: "automation", action: "place", ... })` 将 lane 转为 playlist，
以后即使删除最后一个 clip 也保持 playlist。AutomationClip 保存稳定 id、laneId、trackId、
非负 startBeat、正 durationBeats 和 enabled；authoring 省略时长时采用 lane.lastBeat、
loop.lengthBeats 或 4 beats，snapshot 中总有显式时长。移动/启停走 revisioned builder。
tempo lane 不可放入 Playlist；automation placements 要求 Track 使用 Project 时钟。

Runtime 在控制线程将 enabled clips 和 enabled Tracks 的范围解析成有序、不重叠区间。
每条 lane 最多 1024 placements，工程最多 65536。区间为 [start,end)，source 在片段本地
beat 求值，再应用 lane.loop/lastBeat；同 lane 重叠时较晚起点优先，同起点取字典序较大 ID。
覆盖结束后底层片段按自己的本地时间继续。区间外该 lane 不参与 combine；所有 lane 都
不生效时恢复 snapshot 中的静态参数（未配置则 descriptor default）。播放、seek、离线渲染
使用同一查表路径；片段起止及 source 跳变检查覆盖块内边界。restart chance 的本地 origin
固定为 0，沿用 transport loop iteration，不因 seek 改变片段相位。
带 parts/playback/automationClips 的数据必须声明协议 1.2，TS 和 Rust 均拒绝低版本标记。

Browser 用紧凑行列出根 Pattern、独立放置的 leaf、Sample、非 tempo Automation；拖动产生
落点预览。坐标取实测行 bounds，检查滚动视口和内部窗口遮挡，释放时重新解析目标，
按 accepted builder order 的稳定 ID 查找提交，不猜测过期索引。保存、Undo/Redo 和重开
验证真实源码中的 `.arrange(...)`，不另存隐藏工程。
Automation 资源及片段标签显示所属 Channel/Mixer 名称与参数；片段缩略图在 UI 线程
复用原生 source 求值和 loop/hold 映射，编译缓存跟随 accepted snapshot。缩略图仅代表
片段本地 source，不表示多 lane combine 或当前 transport iteration 的实时输出。

### GPUI 内部窗口

Browser、Piano roll、Mixer、Automation、Plugin Library、实例配置和每个插件 UI 都由同一个
WindowManager 管理，只有一个系统窗口。内部窗口提供层级、点击置前、28 px 公共标题栏、
标题拖动、双击最大化、八方向缩放、最大化/恢复、关闭；Piano/Mixer 可回到停靠区。
窗口限制在标题/传输栏下方实测 desktop 内，宿主缩小时夹紧显示，恢复后保留用户原尺寸。
Arrange 隐藏浮层以回到编排，再点编辑器入口恢复。Escape 取消移动/缩放或未提交的音乐手势，
⌘/Ctrl+W 关闭当前内部窗口。关闭插件视图不创建/销毁 DSP，也不停止播放。

插件声明式面板仍由 GPUI 渲染，初次尺寸采用 manifest，后续刷新保留用户窗口尺寸；
主题跟随宿主。插件库统一使用列表和按需展开的底部详情，类型筛选无需二级菜单；
Automation 使用可横向滚动的紧凑 lane 选择，Piano 工具栏也可滚动。独立窗口不重复显示
大标题或常驻教程。几何/窗口框架/内容组合、Playlist 片段/手势、Pattern 声部选择和
Project automation store 分模块实现；工程规范见 [22](22-engineering.md)。

所有 24 个 source builder 返回不可变 `AutomationSource`，支持 `.replaceRange({start,end,fadeBeats?}, sourceOrPoints)`。
Document 自动捕获已绑定 source 对象，source 范围编辑默认只派生选中 lane 的本地引用，
Shared source 显式改变共享定义。全工程等价比较同时核对其他 lanes、note/插件/路由不变。
重复相同 hard range 绘制合并 wrapper；fade 的叠加保留既有混合语义。

GPUI Automation 面板用原生 CompiledAutomation 绘曲线，按文档 revision/来源缓存。
提供 Points、Draw、Line 工具，可调吸附，指针中心缩放、横向滚动、Linear/Smooth/Step
插值和 Alt 拖动线段生成 Bézier 控制点，Shift 精细定位。顶层 curve 和最外层 hard
replaceRange 的控制点可拖动，不能跨越相邻点；范围内部可插点/右键删点，范围端点不删除。
任意 generator 先画一个 source range 再编辑其中控制点，不把整个生成器采样拆散。
Document 2.0 点数组新增可选 curve，与既有 Curve 合约相同；TS replaceRange 保留并校验
它，省略时继续线性插值。一次抬起一条事务。失效版本/非法范围拒绝，pending 仅由相关
response 清除，Escape 取消未提交手势。源文本立即同步，Save/Undo/Redo 共用工程历史。
图形曲线显示 source 本地域，lane loop 后每轮重复，不是指定 timeline/单次 loop 的编辑。
原生 hard/fade、chance seed-path 透传、边界/分配约束见 [07](07-automation-spec.md)。

## 验证与剩余门禁

CLI 覆盖多文件共享/局部 edit、真实 npm 工厂拆散及副作用、import 遮蔽、确认过期、
依赖 drift、无效草稿、冲突版本、保存重开和 IPC 幂等。目录测试通过真实 C 动态库 helper
验证成功/坏 hash/缺库/越界路径，并证明发现不执行包入口。
`scripts/smoke-daw.mjs` 启动真实 GPUI，执行音符控制器、Save/Undo/Redo，随后新进程重开；
强制 simulated sink，不打开系统输出。聚焦 source-daw benchmark 测完整工程操作。

完整剩余门禁包括完整 Definition resolver/typed runtime Target、保持相位的 split／源偏移与 Playlist 绘制、多个 Playlist clip 的 automation 覆盖编辑、
musical origin、ABI 2、Playlist/Mixer/Sample 完整编辑、lane/轮次 automation 范围、参数试听/
active generation ack、插件安装管理、完整 IDE language service、资产 Save、增量性能。
按设计矩阵逐项完成，不以 Pattern/目录 smoke 代替这些验收。
