# DAW 源码写回复查

重点复查 `b36fab9` 的 Prettier/ESLint 集成及当前 Document → writer → 候选求值 → Save journal
流程，并检查 `a10315f` 之后的 CLI 构建产物。改动仅涉及 Node authoring 层，没有修改音频回调。

## 复现并修复

1. **ESLint 可删除原 import。** 拆散共享引用后，原先被使用的 import 变成未使用；
   原文 lint 干净不能证明候选的全部修复都位于编辑范围内。现在逐条限制 fix 到字符差异中的
   新增区间，保留两处修改之间的原文，并处理 BOM 坐标；差异预算超限时跳过修复。
2. **字符串内容可被格式化改变。** 临时声明折行后的去缩进会删掉模板字符串里的空白；
   重缩进会给普通字符串的反斜杠续行增加空白；Prettier 还会改写 html/css tagged template。
   缩进与去缩进现在共用字符串范围识别，嵌入语言格式化关闭。
3. **配置/编排调用归约会丢注释。** 从 JSON.parse 换成 AST literal reader 后，带注释对象也
   能被读取，旧归约逻辑会丢弃注释。现在保留带注释调用，在外层建立补丁。
4. **格式化后连续修改会增长包装。** 原归约要求调用文本精确相等，换行和尾逗号会令
   `.configure`/`.arrange` 无法归约。现在按 AST 判断，保留 optional-call 和注释边界。
5. **带点号参数无法归约。** writer 输出 `["oscA.level"]`，literal reader 却不接受它；
   连续四次参数修改产生四层 `.withParameters`。现在接受确定的字符串计算属性，仍拒绝
   动态 key、重复 key 和 `__proto__`。已有配置集成测试增加真实 wavetable 参数。
6. **Prettier 配置更新仍读取旧值。** 原 reset 只清服务 Map，Prettier 内部缓存继续生效。
   文档重建现在同时清空两层缓存；测试实际修改 `.prettierrc` 后验证新配置。

上述问题均先通过失败的回归测试复现。新增保存重开测试覆盖连续 DAW 修改、原注释与
模板字符串保留、Save 前磁盘不变、Undo/Redo、保存和新 worker 求值后的 snapshot 一致。
源码格式与插件配置契约同步更新；新增 `diff` 依赖负责有界字符差异计算。

## 验证记录

- 修改前 CLI 基线：33 个文件 / 107 项测试通过；新增测试揭示了原基线没有覆盖的问题。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`：通过。
- `cargo fmt --all --check`：通过。
- `cargo test --workspace`：546 项通过，0 失败，3 项按仓库设置忽略的 UI release microbenchmark。
- `pnpm test`：10 个包 / 89 个文件 / 390 项测试通过。其中 CLI 为 37 个文件 / 122 项，
  包含新增 15 项回归测试及强化后的点号参数集成测试。
- `pnpm --filter @oxitone/cli build`：通过；基准使用此次构建的 dist。
- `git diff --check`：通过。

首次并行 Rust 全量编译与 TS 检查时，`project-controls` 的一个长事务用例超过 30 秒；
Rust 完成后串行 `pnpm test` 使用原超时设置全部通过，没有调大测试超时。
Rust 编译有既存的 `cache_sample` dead-code warning。

`node packages/cli/bench/source-daw.mjs` 正常完成。Apple M4 / darwin 27.0.0 / Node v26.5.0，
100 notes、2 placements、20 plugins，每项 1 次预热 / 10 次测量。工程设置 48 kHz / 128 frames，
device、callback p95/p99、xruns 均为 null。仅记录本次测量，没有用未测的基线宣称性能提升。

| 操作                        | p50 ms | p95 / p99 ms |
| --------------------------- | -----: | -----------: |
| 全工程音符编辑、校验、采用  | 907.03 |      1606.43 |
| 插件配置编辑、校验、采用    | 707.19 |       951.39 |
| Rack 拆散 review 校验       | 724.04 |      1043.35 |
| Dirty journal Save          |  83.93 |        97.84 |
| 效果器重排、校验、采用      | 560.75 |       955.34 |
| 源码与插件投影              |   1.58 |         4.48 |
| Mixer 配置、校验、采用      | 567.60 |       786.48 |
| Playlist resize、校验、采用 | 432.45 |       669.04 |
| 新进程重开                  | 370.51 |       544.01 |

原始日志位于 `/tmp/oxitone-review-{format,lint,typecheck,cli-build,cargo-test,ts-test}.log`，
基准输出位于 `/tmp/oxitone-review-source-daw.json`。这些临时输出和构建产物未加入仓库。

## 范围

验证基线为开始审查时的 `a10315f` 加本轮 CLI 写回修复。结束核对时发现其他流程于约
03:29:51 更新了 `packages/core` 的 clip placement 模块及 `packages/native/src/index.ts`，
另有 README/branding 改动；这些并行改动并非本轮修改，晚于对应测试，不包含在上述通过结果内。

没有执行真实 GPUI 鼠标/键盘操作、VS Code Extension Host 或系统音频设备验收。
源码事务基准不衡量设备 callback 延迟或 xrun。已有多文件 Save 的非协作编辑器 CAS
限制仍按 `17-project-source-session.md` 描述，不因本轮测试而宣称跨文件原子发布。
