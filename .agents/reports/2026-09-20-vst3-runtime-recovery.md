# VST3 运行中能力变化与工程恢复

补齐运行中配置变化的恢复链路：厂商控制器、自动化或 processor 改变总线、延迟或参数表后，
旧实例停止处理并保留当前状态。SDK 显式捕获后更新工程，或 DAW 使用 Use current state，
会按每个实例的恢复状态重新协商、校验、计算 PDC 并换图。候选失败不会替换旧图。

## 实现与边界

- stream 8 的响应标志携带 restartRequired；触发块和后续块必须静音，继续使用旧 Ready
  的总线数、序号和时钟。旧版本明确拒绝，不改写持久化数据。协议、schema 和调用方同步。
- 控制器镜像参数值可能同步改变布局，因此音频 process 前和后都检查 restart 通知。
  参数整体刷新也再次检查，最多 8 轮，不把旧缓冲传给已经改变布局的 processor。
- 冻结关闭编辑器、失败已有手势录制并保留可取消的日志；新录制、改参数、重开编辑器返回
  PluginRestartRequired。重复 capture 和 Seek/Play 不覆盖冻结状态。
- 冻结时读取 component 当前声明的总线；原先读取已准备的 process buffer 会返回旧布局。
  vendor 扩展只用于控制线程的声明查询，不在旧实例上重新分配音频缓冲或协商 PDC。
- 注册 catalog 保留默认类描述。RenderGraph 编译前在隔离 helper 恢复配置并查询实际能力，
  构造只属于候选图、按 instanceId 区分的工厂覆盖。乐器、Channel/Bus/Master insert、
  资源、参数、自动化和输出路由均使用该实例描述；实际 prepare 再核对能力和总线激活。
- 同一插件不同实例可以有不同参数表或 mono/stereo 布局；工厂不得改变 plugin id/version。
  非配置型插件保留借用 registry 的快速路径。新分配、helper 启动和 state 恢复都在控制侧。
- PluginConfig.replaceParameters 完整替换参数表。DAW 接受状态时不再合并已经消失的旧
  ParamID；源码包装会规约，保留工厂执行、instanceId、mix/bypass 和 Undo/Redo/Save。
  vst3Config 允许捕获配置中的新 ParamID，原生层仍按真正恢复后的描述验证。
- Preview 参数详情和自动化标签读取已编译实例的不可变描述，避免继续显示默认 catalog 的
  参数。原生编辑器控制与 source 写入仍通过 Document Service 事务。
- 新模块按配置协商、registry、恢复 conformance 分工。手写实现均约 300 行以内；既有
  validate/state.rs 为 302 行，本轮仅将 descriptor 查询换为实例查询，没有新增状态规则。
  vendor 文件保持上游布局，并在 vendor/vst3-host/OXITONE.md 记录扩展。

## 正确性证据

- `cargo test --workspace`：634 passed、3 ignored，89 suites。忽略项沿用仓库配置。
- 最后一次 helper 边界复查后，`cargo test -p oxitone-vst3-host --features host,stream`
  再次通过 64 项；恢复、配置、transport 和手势 conformance 均用最终 helper 构建通过。
- TypeScript：protocol 59、VST3 17、native 28、core 197、CLI 150 项通过。
  全组首次运行有 4 个 CLI 测试因 30 秒限制超时；相关 3 个文件单独以 120 秒上限重跑，
  7 项全部通过。其余 146 项首次通过；没有将首次运行记录为全绿。
- 原生 fixture 验证控制器、自动化和 processor 三种变化来源，触发块静音、旧布局回复、
  reset 保留状态、旧录制失败、新状态可重复捕获、替换 helper 恢复以及进程回收。
- 工程 fixture 有实际 64 帧 DSP 延迟，并从 stereo 改为 mono、增加 ParamID 7。
  两个并行同类实例只有一个改变；独立参考验证 PDC，对 96,000 个 interleaved 样本比较。
  最大误差为 4.656613e-9；187 个模拟播放块的 xrun、deadline miss、NaN 和 queue drop
  均为 0。后台 block 时间桶 p95=0.524288 ms、p99=1.048576 ms，不是设备 callback。
- 覆盖错误候选保留冻结状态、同类实例隔离、新参数自动化、旧 generation 拒绝、工程保存
  恢复 PCM 完全一致，以及 simulated sink 播放中的图替换。
- 实际 headless Preview 验证 Use current state、第二实例不变、相同 capture 无新 Undo、
  Undo/Redo/Save 和关闭重开后的原生状态；这不是 GUI 视觉或鼠标操作验收。
- macOS arm64/Intel CI 已加入恢复 smoke。未在本轮触发远端 CI，不将配置存在算作 Intel 通过。
- 已安装 VestiGain 的 gain/mix/bypass/serial/Master/state、frame 73 参数事件及
  play/seek/loop/pause/stop 回归通过。1,058 块，xrun、deadline miss、NaN、queue drop
  和 plugin faults 全为 0；后台 block 时间桶 p95/p99 均为 0.262144 ms。

原始证据：

- [运行中恢复、工程 PDC 与实际 Preview 事务](../../benchmarks/results/2026-09-20-vst3-runtime-recovery.json)
- [配置与 4096 参数恢复](../../benchmarks/results/2026-09-20-vst3-recovery-configuration.json)
- [transport](../../benchmarks/results/2026-09-20-vst3-recovery-transport.json)
- [手势与录制](../../benchmarks/results/2026-09-20-vst3-recovery-edits.json)
- [VestiGain 工程](../../benchmarks/results/2026-09-20-vst3-recovery-project.json)

## 检查与性能记录

`pnpm build`、最终 native/Preview 构建、schema 生成、`pnpm format:check`、`pnpm lint`、
`pnpm typecheck`、workspace/fixture 的 Rust 格式检查和 `git diff --check` 通过。
工作区测试后的最终 helper 细化由 64 项 host 测试及上述真实 helper smoke 再次覆盖。

Apple M4 / Darwin 27、48 kHz / 128 frames；`insert_automation` Criterion 基准使用 1 秒
预热、3 秒测量、30 个 samples。本任务的其他 build/test 已结束，桌面程序和系统索引仍在
运行；device、callback p95/p99、xruns 为 null。以下是归一化 batch 耗时，不是 callback。

首次 typed render 中位数 181.517 µs、p95/p99 为 215.469/228.213 µs，录制图编译
p95/p99 为 67.722/71.404 µs。只对这类尾部波动及编译路径做一次有界复测，保留首次结果。
复测 typed render 中位数 174.012 µs、p95/p99 为 184.286/184.531 µs；录制图编译
中位数 48.119 µs、p95/p99 为 50.374/50.896 µs。缓存的历史 baseline 并非受控源码/
环境对照，因此没有据此宣布历史性能门禁清除。

- [首次七项基准与原始样本](../../benchmarks/results/2026-09-20-vst3-recovery-benchmark.json)
- [有界复测与原始样本](../../benchmarks/results/2026-09-20-vst3-recovery-benchmark-repeat.json)

## 仍未完成的接入与验收

当前恢复只接受工程已支持的 mono/stereo 能力；效果器多输出工程路由、更多输入选择、
MIDI 输出与 surround 仍是显式能力边界。每个已配置 VST3 实例编译时先用独立 helper
协商，再准备实际实例，增加控制侧启动成本；未引入会掩盖厂商状态变化的配置缓存。

商业插件资源/授权恢复与负载矩阵、厂商原生窗口实际操作、Intel 实机、签名/公证和发布
仍需要独立验收。所有本轮音频验证均为离线或 simulated sink，没有打开系统输出设备。
本轮没有提交、发布或部署；也未以单插件 smoke 清除历史性能门禁。
