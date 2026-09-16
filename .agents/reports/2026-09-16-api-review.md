# TypeScript public API review

本轮检查公开 authoring/native 入口、参数类型、可变状态与 revision、位置单位和错误边界。
保留 TypeScript authoring / Rust execution 分层；未修改 Rust 实时路径、DSP、ABI 或持久化版本。

## 修复

1. **Clip 可绕过 revision 修改。** PatternClip 的 track/startBeat 和 SampleClip 的 track
   原来是可写字段；直接赋值既不维护 Track membership，也不更新恢复对象的 wire 状态。
   现在提供只读 getter，移动统一使用 relocate 或 Project.arrange。测试覆盖新建与恢复的
   工程、跨轨成员迁移、通道绑定、exclusive end 平移和非法移动不改变 snapshot。
2. **fitBars 使用创建时的拍号位置。** SampleClip 移动到不同拍号后仍按旧 startBar 计算。
   现在每次使用当前 startBeat 与当前拍号表；测试覆盖移动及后续拍号变更。
   Sample 与 SampleClip 分文件，调用者和公开导出同步迁移，不保留内部转发 shim。
3. **输入校验泄漏 ZodError。** configure/arrange 和 native 请求的结构校验统一返回
   OxitoneError/InvalidProject，并在原生调用或 authoring mutation 前失败。对象 snapshot
   编码错误也归一化；现有 OxitoneError 保持原码，JSON snapshot 保留 Rust 版本优先校验。
4. **渲染位置混合单位被静默丢弃。** 原 union 会接受 bar+seconds 或 seconds+frames，
   并删除不匹配字段。位置 schema 现在拒绝混合/未知字段，与既有四选一约定一致。
5. **参数事件允许不精确帧数。** setParameter 的 number 帧位置现要求 safe integer；
   bigint 仍覆盖 u64。无效值在调用 native 前以 InvalidProject 拒绝。
6. **声明与使用方式不一致。** 补齐 authoring 参数和编辑类型、CompileOptions、RenderPosition
   的公开导出；PluginConfig 的 parameters/resources 类型改为只读，与运行时冻结一致。
   SDK 测试从统一入口导入具名类型，tsc 还验证禁止直接写入 clip/config 的负例。

API 指南和规范同步澄清 native wire beat 的有理数形状、frame 的十进制字符串表示，
以及 authoring 层 number/bigint 输入的区别。

## 验证

- `pnpm build`：通过；新增只读声明后另行重建 core。
- `pnpm build:wasm`：通过。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`：通过。
- `pnpm schemas`：通过，生成文件无差异。
- `cargo fmt --all --check`：通过。
- `cargo test --workspace`：546 passed，3 ignored；忽略项为已有 release UI 微基准。
- TS 工作区全部 10 个有 test script 的包：409 tests passed，93 test files passed。
  首次 `pnpm test` 在 Web 包因缺少 dist/oxitone.wasm 中断；补建 Wasm 后定向重跑
  Web、SDK、VS Code 和鼓机包，均通过。没有跳过失败测试。
- 新增 19 项运行时回归测试，另有 tsc 验证的入口类型和只读约束；原有 PluginConfig
  运行时冻结测试保留，并增加编译期写入错误断言。

原始本地日志位于 `target/api-review/`，该目录不提交。全量检查包含工作区原先存在的
CLI 源码回写改动；这些改动不属于本轮 API 提交，保持原状。

## 定向基准

`node packages/core/bench/source-authoring.mjs` 和
`node packages/cli/bench/source-daw.mjs` 均完成。
环境为 Apple M4、Darwin 27.0.0、Node 26.5.0。纯 authoring 基准执行 3 次预热、20 次测量，
覆盖 1k/100k 音符的单音/128 音编辑及 JSON 重建；此轮与部分 Web 测试重叠，不用其时延作回归对比。

Source DAW 基准在测试完成后运行，1 次预热、10 次测量，100 notes、2 placements，
工程 48 kHz / 128 frames。mixer configure p95/p99 为 400.49 ms，playlist resize
p95/p99 为 404.93 ms；包含源码求值、校验和接受流程。这里只记录本机结果，没有性能回归基线。
两项基准均不开音频设备，device、callback p95/p99 和 xruns 为 null，不能视为设备延迟验收。
