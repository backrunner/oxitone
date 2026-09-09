# 真实工程文档服务：实施验证

本批在先前 Pattern writer/MVVM 原语之上补齐实际工程 adapter、磁盘监听、生产源码 Save。
接口、恢复语义和已知边界见 [17-project-source-session.md](../docs/17-project-source-session.md)。

## 已实现

- openProjectPatternDocument 打开真实入口和选定本地模块，自动提供 SourceOwnership、求值器、
  恢复 store 与 watcher，不再要求调用方手写测试 adapter。支持未保存初始文本。
- 对选中 AST 值边界的临时插桩捕获，在新 Node 进程执行实际模块/async factory，保留 this、
  调用次数和源模块 URL。用户代码不添加 ID；npm JS-only 工厂仍以安装路径执行，记录模块读集。
- 内存 edit/订阅/代码草稿、显式拆散、原有 import/export 保留、Save/Undo/新进程重开闭环。
- 外部原子保存与已知依赖变动触发重建；dirty 冲突保留双方文本，明确选择后重建。外部 baseline
  清空整文件历史，避免旧 Undo 覆盖外部代码。真正未保存外部编辑器桥接还未提供。
- 多文件 TS Save 的前后镜像 journal、文件与目录 fsync、协作锁、逐文件路径/内容复核和恢复。
  对真实子进程执行 SIGKILL，确认第一文件已发布、第二文件未发布；重开恢复整套旧代。
  第三种外部内容保留并拒绝自动覆盖；禁止依赖目录和非 UTF-8 源码写入。

## 验证

- pnpm lint、pnpm typecheck、cargo fmt --all --check、cargo test --workspace 全部通过。
- CLI 全量 12 个文件、45 项测试通过；其中本批真实工程 session 5 项、Save/恢复 6 项。
- 既有 Preview/bundle 回归通过，Rust 无实现修改，未打开系统音频设备。
- git diff --check、修改契约的相对链接和代码围栏检查通过。
- Rust 全量日志：`/tmp/oxitone-project-source-cargo-test.log`（本地临时日志，不是提交产物）。

## 聚焦基准

CLI build 后运行 source-session.mjs 与 source-writing.mjs。先完成全量检查，再单独运行基准，
以下仅采用独立运行的结果。Apple M4、darwin 27.0.0、Node v26.5.0。
source-session：100 音符、1 次预热、10 次测量；10 样本的 p95/p99 均接近最大值。

| 操作 | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- |
| 完整候选求值与文档接受 | 236.234 | 244.206 | 244.206 |
| journal/fsync 源码 Save | 84.979 | 93.187 | 93.187 |
| 恢复检查与新进程重开 | 208.150 | 235.365 | 235.365 |

source-writing：3 次预热、20 次测量，1,000 音符：

| 操作 | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- |
| sparse 文本回写 | 1.577 | 2.987 | 3.116 |
| 拆散并新增 import | 18.533 | 21.219 | 24.577 |
| literal Note 回写 | 22.612 | 23.531 | 25.485 |

session 耗时包含源码/依赖 I/O 和新进程，不包含 GPUI/Rust prepare/播放图切换。source-writing
仅测 authoring/AST。设备、sample rate、block size、callback p95/p99、xruns 均未测，记 null。
这些结果不证明 60 Hz 交互或音频实时预算；GPUI 仍需即时本地投影和手势节流构建。

## 剩余落地门禁

全工程自动来源索引/多边界选择、完整依赖负证据和副作用追踪、共享影响范围与 Rust prepare；
GPUI request/response/event 编辑协议和视图绑定；新模块/资产多文件事务、冲突三方合并与
通用未保存编辑器桥接；arrangement/automation/effects/rack 与概率/窗口语义；插件 ABI 2
和 GPUI 插件管理器。当前不能把单 Pattern 服务接口称为完整 DAW 已交付。
