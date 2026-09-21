# VST3 SDK 继续验证

本轮没有改变实时协议或 GUI 行为；针对上一轮调度与 IPC 修复重新跑了跨包测试和实际工作台
smoke，确认修复没有破坏离线 SDK、DAW 文档服务或预览界面。

另外补强了 native configuration 边界：Rust 在加载插件前独立验证 class ID、SHA-256、
base64 解码和 4 MiB opaque state 限制，不再只依赖 TypeScript schema。

## 结果

- `pnpm test`：445 个 TypeScript 测试通过，所有 workspace 子包退出码为 0。
- `cargo test -p oxitone-preview`：73 通过、3 个明确忽略。
- `cargo test -p oxitone-vst3-host --features host,stream`：42 通过，包含 malformed
  configuration identity/state 和 oversized decoded state 测试。
- `scripts/smoke-vst3.mjs --gui`：实际 VestiGain bundle/class 通过 Inspect、参数设置、
  离线 WAV、PCM 校验、source save/reopen；输出为模拟音频，无系统设备。
- stream、固定调度、managed lifecycle smoke 仍通过：2,208 块有序往返、281,600 帧固定
  延迟校验、163,840 帧 32 epoch 实例切换，所有 helper 回收。
- `git diff --check`、`cargo fmt --all --check` 和前一轮的 format/lint/typecheck 均保持通过。

## 当前边界

`@oxitone/vst3` 已完成可选本地 helper、精确 class/hash 校验、状态/预设、离线渲染和有界
Rust stream；DAW/GUI 只提供离线工作台与冻结音频导入。VST3 仍未进入 Engine RenderGraph，
没有真实设备 callback、PDC、seek/loop 连续性或厂商 editor 验收。这些属于后续实时宿主阶段。
