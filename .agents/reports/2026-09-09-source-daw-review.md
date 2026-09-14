# 2026-09-09 代码 / DAW 全实现审查

后续[发布前清理](2026-09-09-prerelease-cleanup.md)已删除单 Pattern 文档、journal 1 兼容与
旧式 requestId 路径，projectRoot 现为必填。下文相关兼容说明与旧基准为清理前历史证据。

结论：现有实现具备可验证的代码 ↔ GPUI ↔ VS Code 编辑、源码保存重开闭环，但**不能通过完整交付验收**。
本次发现并修复 7 项缺陷，涉及未保存文本丢失、异步竞争、长会话不可写以及源码插桩/import。
通过回归只说明已覆盖场景成立，不构成“所有预期均已落地”或“没有任何 bug”的证明。

后续源码创建增量另修复了此前入口未覆盖的完整生命周期缺陷，详见下文；本文件后半部分
较早的测试数量与 VSIX hash 是历史证据，不能当作最新工作区的打包验证。

## 后续增量：新模块、文件历史和编辑器命令

- 新文件不再依赖磁盘上的模块才能求值。独立 `project-source-loader.ts` 管理已登记 overlay，
  相对 import/re-export 支持 `.js`→`.ts`、`.mjs`→`.mts`，保留与保存后相同的解析优先级；
  Undo 已保存 TS shadow 时可恢复既有 JS fallback。不存在的已导入模块记录负读证据。
- journal 2 用 null 表示文件不存在，已有空文件保持空串；创建、修改、历史删除共用前后镜像。
  ProjectDocument 在发现/读取源码前恢复日志，修复磁盘已回滚但内存仍执行半发布文本的问题。
  创建采用 no-clobber hard-link；日志记录 staging identity，恢复校验 inode 并清理 link/unlink
  间中断的双链接。version 1 的空文件回滚不会被删除。
- 创建先检查预算、根目录、父目录、嵌套 package 与文件别名，再复核 revision/generation 登记；
  拒绝已有磁盘路径和并发过期创建。Save 比较文件集合与内容，Undo 后保存会删除新文件，Redo 重建。
  watcher 检查未落盘路径的外部创建；空文件碰撞不会被当成合法不存在 baseline 覆盖。
- `DocumentView.projectRoot` 为 2.0 的可选扩展字段；VS Code 不再从 `files[0]` 的 dirname 猜路径。
  订阅首次同步通知的清理改为 microtask，避免 `unsubscribe` 尚未初始化导致创建等待超时。
  新建、插件与项目命令互斥，命令期间保留键入，完成后按新 revision 提交；插件等待延长到 150 秒。
- GPUI Add package 与目录搜索分开保留输入，非法包名或工程忙碌显示诊断，避免丢输入和重复提交。

新增 `source-creation.test.ts` 覆盖真实模块求值/音符写回、Save/重开、空文件、撤销删除、
旧 journal、SIGKILL 与发布链接恢复；`editor-races.test.ts` 验证请求互斥和后续按键提交。
真实 VS Code Extension Host 已验证 Create Source → 未保存 import → DAW 音符编辑 → Save，
以及创建文件的 tab 保持打开时 Undo/Redo。所有验证使用离线/模拟输出。

本增量不关闭通用 rename/delete、新目录/资产写回、tsconfig paths/条件导出的完整虚拟解析、
插件 ABI 2 或包安装的整工程回滚。复核还确认 `repair` 当前把依赖名传给 pnpm workspace filter，
不能算可靠的逐包修复；manifest 恢复仍吞掉恢复异常、未覆盖 workspace 根 lock 和 node_modules。
这些插件生命周期缺口仍需实现和故障注入验证，不能以任务命令入口存在作为验收通过。

本增量验证：全仓 `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`git diff --check`
通过；`cargo test --workspace` 为 516 passed、0 failed、2 ignored（既有布局/编辑手势基准）。
CLI 完整 91 tests/29 files、protocol 39 tests/5 files 通过；最后补齐目录 index 解析后，源码
创建专项 12 tests 全部通过，并再次通过 lint/typecheck。Extension Host 1.96.4 实测通过，
日志 `target/source-creation-vscode-host.log` 包含全部已有同步/冲突/生产启动场景和新文件链路。

开发 VSIX 已刷新为 `target/oxitone-vscode.vsix`，6 entries；归档内 bundle 和 README
与工作区产物逐字节一致，SHA-256 为
`73c4a625b4bc7d2c3fbb936ede92e36aa8276e0624a433e68230614a3008a9ec`。
打包器仍提示 repository 字段缺失和 bundle 大小；未发布。

聚焦 source-session 基准为 Apple M4/Node 26.5.0、100 notes、1 warmup + 10 iterations：
候选接受 p95/p99 172.97 ms、fsync Save 69.52 ms、恢复重开 170.05 ms。
完整 Project 基准也单独运行；首次与 VSIX 打包重叠时检测到 pnpm-lock.yaml 变更并返回
SourceChanged，未把该轮计为性能样本。待打包和锁文件恢复完成后重跑；最新数据见
`target/source-creation-daw-final.json`，其他日志均为 `target/source-creation-*`。
该小样本不关闭大工程或音频 callback 门禁；没有测量设备、callback p95/p99 或 xruns。

审查对象是当前工作区的全部 source/DAW 实施增量，包括未跟踪文件；没有提交、发布或改动
用户依赖包。按 oxitone-guard 审读 TS authoring/protocol、Document Service/源码 writer、
编辑器客户端/VS Code、GPUI、Rust instance target 与 automation range 路径，并核对
[完整交付矩阵](../designs/source-daw/06-delivery.md) 与 [MVVM/本地化设计](../designs/source-daw/07-mvvm-and-localization.md)。

## 已修复缺陷，按影响排序

| 优先级 | 触发与修复前影响                                                                                                                 | 修复与回归证据                                                                                                                                                                                                                                                   |
| ------ | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1     | 服务已接受编辑器 B，但磁盘仍为 A；服务重启加载 A，客户端把 B 当作共同 baseline，静默提出覆盖为 A，丢失未保存输入                 | `editor/buffers.ts` 给 baseline/apply 保留 session 身份；不同新服务文本形成显式冲突。VS Code 持久化 baseline 的身份。`editor-races.test.ts` 与真实 Extension Host 验证保留 B、重连后 Save 明确因冲突拒绝、显式同文本收敛后可保存                                 |
| P1     | 256 个结果和 4096 个 retired request IDs 耗尽后，后续所有请求包括 Save 都返回 BudgetExceeded，长时间工作不可继续                 | `document-request-history.ts` 用最多 64 个客户端的单调序号保护过期重放，缓存 256 个结果；legacy 限额与生产 stream 隔离。GPUI/editor 均迁移请求 ID；5000 次连续请求后 Save 成功，旧 Save 不重放，重复 Save 幂等；另测 legacy 耗尽、身份冲突、非法序号和客户端限额 |
| P2     | 宿主正在异步应用远端 B 时收到 C，旧代码把 proposal 换成 C，实际 B 的 change/ack 被当成用户竞争或无效确认                         | `EditorBuffers.reconcile` 保留已发给宿主的准确 proposal，确认 B 后再提出 C；测试模拟被延迟的宿主应用                                                                                                                                                             |
| P2     | 一个 socket batch 含 B event、B accepted response、C event，客户端只看最新 C，丢掉 B 被接受的事实，错误产生冲突                  | `EditorDocumentSession.pump` 根据相关成功 response 单独推进 B baseline，不能依赖最新 event 恰好等于 B；真实 Unix socket 分帧/批处理回归                                                                                                                          |
| P2     | directive 与 import/声明写在同一行，新增 import 的位置越过实际编辑表达式，拆散报错或产生非法候选                                 | `imports.ts` 在完整 directive 后扫描 trivia，保留同行注释/CRLF/shebang，不越过下一条可执行语句。同行源码新用例和既有格式保留用例均通过                                                                                                                           |
| P2     | 合法作者源码声明 `globalThis`，临时插桩在作者作用域调用同名局部值，工程无法求值                                                  | `project-capture-module.ts` 在独立虚拟模块词法作用域读取 hook，选择不冲突的 import alias。完整 Project 与选定 Pattern 两条求值路径共同使用；真实执行、编辑、Save 验证，临时 hook/import 不进入源码                                                               |
| P2     | 本地模块只导入 npm 编曲 helper，工程只安装 `@oxitone/core`，拆散却新增未安装的 `oxitone` import，候选 build 失败                 | import planner 从选中源文件解析可用 SDK 包；`source-review.test.ts` 覆盖该布局的 npm 拆散、保留 entry/export、保存重开和 dependency 字节不变                                                                                                                     |
| P2     | 工程发现与磁盘 watcher 使用无界 `readFile`；超大文件/多文件工程会在上层预算拒绝前先分配超限内存，甚至 `Promise.all` 并行放大峰值 | `project-files.ts` 新增分块 `readSourceText`，先 stat、再按 8 MiB cap 读取；项目发现串行累计 32 MiB，`project-disk.ts` 共用读取器；ProjectDocument 删除二次无界读取。5 个 source ownership/budget tests 覆盖单文件与聚合上限                                     |

相关实现位置（相对仓库根目录）：

- `packages/cli/src/editor/{buffers,session,connection}.ts`
- `packages/editor-vscode/src/{linked-session,presentation}.ts`
- `packages/cli/src/source/{document-dispatch,document-request-history,imports,project-capture-module,project-instrument,project-evaluator,project-evaluation}.ts`
- `packages/cli/src/source/{project-files,project-disk,project-document}.ts`
- `apps/preview/src/document_ui.rs`

修复前失败日志保留于 `target/review-{editor,requests,source}-before.log`、
`target/review-import-before.log`；选定 Pattern 的旧 hook 失败见
`target/review-cli-intermediate.log`。该中间轮还出现机器外部负载下 1 秒 evaluator 用例超时，
另一次 narrow test 使用 vitest 默认 5 秒而超时；未以放宽生产限制处理。修复后使用仓库已有
30 秒 test 配置，选定 evaluator 的 1 秒 runaway 限制保持原样，相关用例通过。
新增 import 初版曾破坏 directive 同行注释，完整 CLI 回归发现后补齐并复测。

## 预期能力核对

“部分”表示已有真实实现，但该领域的完整验收尚不能关闭。每项均按当前目标判断，
不把最初的只读 Preview 或 ABI 1 基线等同于本次完整双向编辑目标。

| 领域               | 当前已实现/证据                                                                                                                                                                                                                      | 尚缺的验收或实现                                                                                                      |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| chord              | 来源 degree/voice、截顶同音区分、单音字段/增删；core source 与 writer 测试                                                                                                                                                           | 全六 quality/options × GPUI 手势组合矩阵、生成器规则面板                                                              |
| arp                | 四 order 的既有语义；输出 step 编辑、删除留空拍、插音不重算旧力度、seed 输出重开                                                                                                                                                     | 完整 octave/重复输入/规则面板交互矩阵                                                                                 |
| 组合               | chord → arp → concat/repeat/transpose source DAG，definition/单 placement 隔离和跨文件共享                                                                                                                                           | 全部四种层次；source iteration 不等于播放 Clip loop 的单轮例外                                                        |
| 源码               | literal/变量/alias、sync/async factory、本地/外部 helper 捕获、最小表达式 patch                                                                                                                                                      | 任意循环/重复执行引用无法唯一隔离时明确拒绝；完整依赖/definition resolver 尚缺                                        |
| MVVM/import/export | linked dirty buffer 实时同步、版本条件、防回环、冲突；运行时 import 别名/type-only/遮蔽/格式、出口文本保留                                                                                                                           | 条件导出/循环/全部模块布局的 resolver；普通 `file:` tabs 仍仅 disk watch                                              |
| npm 本地化         | Pattern 与串联效果器 rack 具体候选 review/确认、调用侧展开、其他引用与依赖源码不变                                                                                                                                                   | 资源/state、并行 rack、宏/绑定迁移；保留原工厂副作用的拆散仍会执行原依赖                                              |
| edit 归约          | 重复绝对拖动归约、literal 直接回写、insert/remove 抵消、expect/歧义拒绝                                                                                                                                                              | 所有结构/时间域 edit 的归约和长期复杂表达式预算                                                                       |
| Arrangement        | 既有显示、播放、Pattern source 派生                                                                                                                                                                                                  | Clip 移动/替换/跨 Track、相位保持 split/window、Playlist 编辑、单轮编辑                                               |
| automation         | 全 24 builder 可 `replaceRange`，source/单 lane 引用范围、唯一 Playlist clip 的 clip-local range、hard/fade golden、loop/chance seed-path 保留、GPUI 实测                                                                            | 多 clip winner/覆盖编辑、统一共享 automation DAG、完整规则编辑面板                                                    |
| 随机               | 固定 seed arp 与 source 规则稳定；当前不安全的概率 note/placement 图形编辑明确拒绝                                                                                                                                                   | 新 musical origin/random-v2、移动/复制/重排/窗口的全部随机不变性                                                      |
| Sample             | 既有 SDK/Rust 播放、sample edits/fit/tempoSync                                                                                                                                                                                       | DAW trim/fit/split、相位窗口、grid/onset 单片编辑与 trigger 迁移                                                      |
| regions            | 既有 grand/soft/multisampler 声音生成                                                                                                                                                                                                | 图形单区/多层拆散、资源/defaults/键力度边界的本地回写                                                                 |
| 插件实例           | engine 1.1 instanceId、plugin/effectHost 分域参数、共享 config 的两个实例独立、native 校验/离线测试                                                                                                                                  | 最终 AuthoringDocument/EngineProject 2.0；纯 Definition/Config/Instance resolver                                      |
| 插件 ABI           | 真实外部 C 插件 ABI 1 参数/验证与 npm fixture 闭环                                                                                                                                                                                   | ABI 2 resources/state/events、迁移与真实 conformance fixtures；core 仍依赖 native                                     |
| 面板               | 通用参数、mix/bypass、声明式布局回退、已有详情窗口                                                                                                                                                                                   | native UI companion gesture/config 事务与 generation、连续参数试听                                                    |
| 重排替换           | 以实例身份绑定 automation；GPUI reorder 回写 `orderEffects`，不在 TS 写 ID                                                                                                                                                           | GPUI 添加/替换、升级参数/state 迁移失败的整工程回滚                                                                   |
| 管理器             | 20 builtin + 已安装 npm 静态目录、搜索、版本/平台/hash/签名策略、独立 helper Verify、使用位置；Document install/upgrade/uninstall/repair task 与 GPUI manager 操作；GPUI/VS Code 可添加未安装包；失败恢复 package.json/lockfile 快照 | 多平台包解析、node_modules/state 的完整迁移回滚与 1k 条目性能                                                         |
| 保存/恢复          | 现有登记文件、多文件 fsync journal/锁、SIGKILL 恢复/第三方 hash 冲突、VS Code Save/autoSave；新增 TS 文件受根目录/扩展名约束并纳入 journal 创建恢复，VS Code 可创建并打开 linked buffer                                              | 新资产、更多磁盘满/权限/故障点；server-only 未保存修改未 crash-journal；完整 hot-exit/window reload 未实测            |
| 并发/进程          | 外部 atomic save、防过期候选、编辑器版本竞争、同文本收敛、断线重连、队列/过期请求/幂等；新增保守逐行三方合并                                                                                                                         | 语义级三方合并、rename/动态新增依赖、所有故障及多窗口组合                                                             |
| 重开/音频/性能     | CLI/GPUI 保存后全新求值；Rust offline/native simulated/Web Wasm parity；本次 source benchmark                                                                                                                                        | 全量缓存删除 release fixture、全部 block/tempo/窗口新模型对拍、active graph ack/实例移交、大工程及 callback/xrun 证据 |

高阶 API 盘点覆盖类别沿用 [入口审计](../proposals/2026-09-08-higher-order-edit-audit.md)，
该历史文档中“还没有 repeat/concat/slice 或 replaceRange”的描述属于原始基线，当前已增加。
24 个 automation 入口为 constant/curve/polyline/line/gate/chance/wave/sine/cos/triangle/saw/
ramp/square/map/clamp/invert/quantize/scale/offset/mix/add/multiply/min/max，core range test
逐一覆盖。Sample fit、slicer、grand/soft、preset/applySettings、routing/clock 有既有 SDK
不意味着已经拥有图形回写；上表明确保留这些未交付项。

## 检查与集成证据

- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`git diff --check` 通过。
  扩展消费 CLI dist 类型，首次 typecheck 使用旧声明失败；重建 CLI 后全仓通过。
- `cargo test --workspace`：497 passed、0 failed、1 ignored（既有 GPUI 布局 benchmark）。
- CLI 全套 66 tests/25 files 通过；随后新增 helper-only import 与 stream budget 用例，
  对最终 writer/evaluator 运行 14 tests/3 files、request history 运行 2 tests/1 file，全部通过。
  core 169、protocol 39、VS Code 单元 1、web 9 tests 通过。
- VS Code 1.96.4 真实 Extension Host：未保存代码求值、可见/后台 tab 远端更新、实际版本竞争、
  journal Save/autoSave、项目 Undo/Redo、旧服务接受但未保存的重启冲突、离线输入重连、依赖写保护。
  重启场景等待**扩展自身**重连，再断言 Save 因 editor conflict 拒绝，避免把断线拒绝误算为冲突保护。
  另跑 production ProcessExecution → CLI → headless simulated GPUI。
- 重建 GPUI 后，真实 `smoke-daw.mjs` 的 configuration 与 automation 模式均通过：
  音符手势、外部 C 插件/rack 本地化与参数/重排/typed binding、source range、Save/Undo/Redo，
  随后新进程从 TS 重开，其他 placement/实例不变、依赖文件 hash 不变。
- 音频验证均为 offline/simulated；没有打开系统音频输出。Web 测试使用 Wasm/native 离线 PCM，
  本次没有新增真实浏览器 AudioContext 或硬件 callback 测量。

本轮在上述审查之后又补充了 `upgradePlugin`：协议、CLI Document dispatcher、GPUI wire/manager
和 lifecycle 回归均通过；它只执行版本化 package-manager update，不执行插件参数/state 迁移，
因此 ABI 2 migration/rollback 仍保持未完成状态。

同时补充了 Project Document 的 `merge` 冲突解决：基于 baseline/draft/disk 生成不重叠行级
hunk，成功后重新求值；重叠替换和同位置不同插入会返回 `EditScopeConflict`，不会自动覆盖任一侧。

日志：`target/review-{lint,typecheck,fmt,rust,cli,core,protocol,vscode,web}.log`，
`target/review-source-final.log`、`target/review-requests-final.log`、`target/review-vscode-host.log`、
`target/review-gpui-{configuration,automation}.log`。旧较慢/失败证据保留，不用最后成功覆盖结论边界。

## 性能与交付门禁

`target/review-source-bench.json`：Apple M4、Darwin 27.0.0、Node 26.5.0，48 kHz/128 frames，
100 notes、2 placements、20 plugins，1 warmup + 10 实测；此样本量的 p95/p99 均为最大值。
本次等待自己的测试/build 完成后只运行一次，系统仍有其他任务，不声称隔离性能实验。

| 操作                        | p50 ms | p95/p99 ms |
| --------------------------- | -----: | ---------: |
| 音符提交、完整候选校验/接受 | 211.01 |     231.92 |
| 配置提交、校验/接受         | 197.46 |     224.40 |
| rack 拆散候选/校验          | 200.23 |     251.86 |
| dirty journal Save          |  60.26 |      84.93 |
| 效果器重排/校验             | 276.25 |     500.27 |
| source/plugin 投影序列化    |   1.66 |       2.24 |
| 新进程重开                  | 232.40 |     240.33 |

效果器重排 p95 超过小工程局部 edit 的 300 ms 预算，性能门禁**未通过**。保留此前
`daw-instances-source-bench-serial.json` 更慢样本和 `daw-vscode-source-bench.json`，
不选择最快一轮作为完成证据。本次未改变 callback 路径；device/callback p95/p99/xruns 均未测，
控制侧毫秒数不能替代音频设备指标。100k notes/1k placements/100 lanes 与 1k catalog 仍未测。

后续已针对 worker 启动和读集复核优化，最终 source-daw 排序 p95 为 157.43 ms，
40 次专测 p95 为 258.92 ms，通过小工程 ≤300 ms 目标；专测 p99 仍有 301.85 ms。
上述旧记录保留为历史基线，完整数据与边界见 [效果器性能报告](2026-09-09-effect-performance.md)。

重建本地 `target/oxitone-vscode.vsix`，包含此次修复；未发布。归档检查确认 bundle 与当前
已测 build 字节一致，6 个 ZIP entries，无 node_modules/native 二进制；证据见
`target/review-package.log` 和 `target/review-package-contents.json`。
VSIX SHA-256：`8c2716bc5531c7a6024e884b53c593e6405f3b72e0e104f1aabc8b193c2a7bc5`。
vsce 的 missing repository、bundle size 提示原样保留，打包 exit 0；不等于 Marketplace 发布验收。

源码 I/O 的读取前预算已在后续增量中补强：`project-files.ts` 和 `project-disk.ts` 使用
分块读取器，单文件 8 MiB、工程 32 MiB 在读取阶段生效；source ownership 测试覆盖单文件
与聚合上限。仍未做巨型文件 OOM/压力矩阵，因此这不是完整性能证明。
同样未关闭 BOM、完整 hot-exit/window reload、动态依赖/rename 和大工程矩阵；这些没有被
计作本次已复现修复的 7 项 bug。

建议实现顺序维持原设计：先完成纯 authoring/协议/随机与时间域，再接结构/采样 writer；
插件 resolver/ABI 2 与安装迁移任务配套推进，最后补完整 GPUI/IDE 操作、增量试听和发布门禁。
完整交付前，必须逐项关闭上表，而不是继续用一个已通过的音符/参数 demo 表示全部能力完成。
