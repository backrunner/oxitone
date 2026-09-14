# Pattern 文档与项目内拆散：实施验证

本文保留第一批文档原语的验收状态。后续生产 evaluator、Save/恢复和 watch 的增量见
[真实工程 session 验证](2026-09-08-project-source-session.md)。

本批实现位于 `packages/cli/src/source`，公共能力与限制见
[16-pattern-document.md](../docs/16-pattern-document.md)。所有测试均无系统音频输出。

## 新能力与依据

- SourceOwnership 明确源文件所有权，拒绝依赖、生成目录、项目外路径、硬链接、重定向
  符号链接与未登记的 nested workspace package；调用时复核，不只在打开工程时检查。
- PatternDocument 提供同一 text/source revision 的订阅、代码草稿、语义 edit、Undo/Redo。
  无效代码保留 last good，旧 evaluation/旧候选/关闭后完成均不覆盖当前状态，listener
  错误不回滚已提交的音乐。该层尚无 GPUI、watch 或外部编辑器桥接适配。
- 拆散先产生可审查 before/after 和音符数量/生成联动丢失说明，完整候选求值后才可
  确认；取消、复制计划、重复/过期确认和依赖内容变化均不能修改文档。
- Pattern import 能复用 named/namespace 运行时绑定、区分 type-only 和 shadowing；
  需要时增加唯一 value import，保留导出和原依赖声明。拆散后 literal Note 可直接编辑。
- 集成测试在临时项目安装目录构造 JS-only npm 包（无 TS/sourcemap），调用真实 esbuild
  和全新 Node 进程求值。局部派生和拆散均只改变指定引用，另一个 shared export 不变；
  包 manifest/JS 字节保持，保存测试源码后重开结果一致。进一步显式移除其他使用者和
  import 后，独立 literal 结果在删除生成包的环境下仍能执行。

## 检查

`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`cargo test --workspace` 通过。
CLI 新增所有权 3 项、本地化/import 6 项、文档状态/并发 7 项、npm 集成 2 项，共 18 项。
最终 CLI 全量 10 个测试文件、34 项全部通过。
旧 Pattern writer 的 6 项与原 Preview/bundle 等测试继续覆盖原有行为。
Rust 本次无实现修改，workspace 回归使用原有 offline/simulated sink 测试。

## 聚焦性能

`pnpm --filter @oxitone/cli build && node packages/cli/bench/source-writing.mjs`。
Apple M4 / darwin 27.0.0 / Node v26.5.0；3 次预热、20 次测量。20 次样本的 p99 接近最大值。

| 音符数 | 操作              | p50 ms | p95 ms | p99 ms |
| ------ | ----------------- | ------ | ------ | ------ |
| 100    | sparse 文本回写   | 1.436  | 2.540  | 4.292  |
| 100    | 拆散并新增 import | 3.617  | 5.271  | 5.671  |
| 100    | literal Note 回写 | 3.071  | 4.793  | 4.877  |
| 1,000  | sparse 文本回写   | 1.673  | 3.996  | 4.180  |
| 1,000  | 拆散并新增 import | 19.682 | 22.445 | 26.020 |
| 1,000  | literal Note 回写 | 24.512 | 26.124 | 27.584 |

测量包括 authoring 校验、AST/局部作用域与打印；不包括用户工程执行、依赖 I/O、
文件发布、GPUI 绘制或 Rust prepare。设备、sample rate、block size、CPU utilization、
callback p95/p99 与 xruns 未测，脚本输出 null；不能据此声称端到端延迟已经达标。

## 当前限制

只有单文件、单个可定位 Pattern 边界的文档原语，完整模块 evaluator 仍由 adapter 提供。
当前生产代码不执行自动来源插桩、不写项目文件，尚无多文件保存/恢复日志。
拆散保留原调用/import，仅替换绑定引用；直接调用、属性 getter、re-export 的定义改写、
概率/窗口保真、效果器组合和工程上下文验证待后续实现。GPUI 编辑、ABI 2 与插件管理器
未交付；不能将测试的手工文件发布视作生产 Save 或已完成代码/DAW 实时连接。
