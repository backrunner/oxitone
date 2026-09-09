# 效果器排序控制延迟

本轮针对代码/DAW 效果器重排的源码事务性能，未修改 Rust DSP 或音频 callback。
此前 source-daw 小工程排序 p95 为 447.65 ms，超过交付矩阵的 ≤300 ms 目标。
最终同一完整工程基准为 157.43 ms；40 次专用基准为 258.92 ms，两项 p95 通过该目标。
专用基准 p99 仍为 301.85 ms，不将该结果解释为每次提交都低于 300 ms。

## 实现

- 发布构建的 worker 直接以 JS 启动，先加载协议等宿主模块，再在执行用户工程前加载
  tsx；避免让宿主 JS 图经过 TypeScript loader。源码 worker 仍在启动时预载 tsx。
  npm TS/TSX/CJS 保留同一 loader，factory 每次仍在新进程执行；无音乐结果缓存或进程复用。
  插件验证 worker 的发布 JS 直接运行，不加载不需要的 TS loader。
- capture/checkSourceReads 每批最多并行 16 个文件，保持真实路径和 SHA-256 复核，
  不依据 mtime/size 复用结果。失败时排空当前批次，按输入顺序报告首个失败，不启动下一批。
  改写前、候选求值后、原生校验后、接受前和 Save 的既有复核全部保留。
- module hooks 每次求值按目录扫描一次祖先 package/lock；记录缺失文件的负证据，
  扫描后出现新 manifest/lock 会使候选失败。目录缓存不跨求值保留，私有临时 bundle
  目录不登记会在求值后失效的负证据。其余任意 I/O 与 tsconfig 负证据仍按规范列为限制。
- `source-timing.ts` 的内部 diagnostics channel 仅在订阅时计时。writer/native 校验的
  计时留在各自模块，ProjectDocument 只包围现有排序事务；不增加第二个文档状态所有者。
  专用基准记录各阶段和全部样本，不把嵌套阶段重复相加。

## 测量

Apple M4，Darwin 27.0.0，Node 26.5.0，Rust 1.98.0 / LLVM 22.1.8。
同一工作区原生构建；100 notes、2 placements、wavetable、2 delays（default / feedback=.6），
插件目录 20 项，48 kHz / 128 frames。所有基准无设备；callback、音频 CPU、xrun、峰值内存未测。

最终 effect-order：5 次预热 + 40 次测量，单位 ms。

| 阶段 | 优化前 p50 | 优化前 p95 | 最终 p50 | 最终 p95 | 最终 p99 |
| --- | ---: | ---: | ---: | ---: | ---: |
| AST writing | 1.44 | 2.37 | 1.47 | 3.28 | 3.54 |
| 全事务累计读集复核 | 26.60 | 36.41 | 21.07 | 30.63 | 56.03 |
| bundle | 5.23 | 8.36 | 6.42 | 10.77 | 35.16 |
| worker | 147.95 | 222.96 | 139.98 | 188.59 | 225.90 |
| native validation | 1.18 | 1.74 | 1.33 | 2.28 | 3.29 |
| 排序事务总计 | 186.54 | 282.51 | 175.33 | 258.92 | 301.85 |

最终 source-daw：1 次预热 + 10 次测量，排序发生在音符、配置、rack review、实际 dirty Save
操作之后。p95/p99 均为十个样本的最大值，不能据此推断更长使用期间的尾延迟。

| 操作 | p50 ms | p95/p99 ms |
| --- | ---: | ---: |
| 音符提交/校验/接受 | 160.27 | 194.76 |
| 配置提交/校验/接受 | 153.16 | 160.61 |
| rack 拆散候选/校验 | 158.35 | 182.17 |
| dirty journal Save | 44.77 | 62.72 |
| 效果器重排/校验/接受 | 148.96 | 157.43 |
| source/plugin 投影 | 1.34 | 1.72 |
| 新进程重开 | 159.18 | 163.93 |

原始数据与所有有效中间实验归档于
[effect-order JSON](../../benchmarks/results/2026-09-09-effect-order.json)。其中仅 compile-cache
实验启用 NODE_COMPILE_CACHE；实现没有默认启用该缓存，最终两轮也没有启用。
中间实验存在更慢和更快的结果，全部保留：不能拿较快的 178.59 ms 代替最终 258.92 ms。
基准期间无本任务的并行构建/测试，但非随机配对实验仍受调度、GC、文件系统噪声影响；
新增负证据也改变了读集工作量。不将 447.65→157.43 ms 视为跨设备固定倍率。

## 验证与范围

- CLI 92 tests / 29 files 全部通过。新增回归覆盖跨批次字节变更、恢复 mtime 的同长度改动、
  同内容 symlink 重定向、缺失文件出现、多失败顺序和 4096 读集预算。
- 源码及发布构建 worker 均验证 npm `.ts` 导出、`.js`→`.ts` 相对导入、TS 参数属性，
  工厂每轮仅执行一次、Undo 重求值、外部改动拒绝旧事务并在同步后采用新配置、
  求值期间新增 lockfile 拒绝。现有排序/自动化绑定、npm C rack 拆散、Save/重开、超时/
  取消、崩溃恢复回归均通过。
- 真实 VS Code 1.96.4 Extension Host 通过生产 CLI 启动、未保存同步、DAW 回写、共享
  Undo/Redo、Save/autoSave、重启冲突和依赖保护；GPUI 使用 simulated sink。
- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`cargo test --workspace` 通过；
  Rust 516 passed、0 failed、2 个既有基准 ignored。日志在 `target/effect-performance-*`。

这是小工程效果器源码事务的性能改进。100k notes/1k placements/100 lanes、长会话内存、
连续参数试听、实时设备/DSP 性能与全部 DAW 交付矩阵仍须各自验收。未发布任何包。
