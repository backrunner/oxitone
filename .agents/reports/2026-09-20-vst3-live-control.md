# 常驻 VST3 实例控制与原生窗口

本轮将厂商窗口、参数修改和 state 捕获接到同一个原生处理实例。新增 Rust
`Session::controller()`，保持现有音频端口不变；DAW 的已插入实例入口尚未迁移到这个控制面。

## 实现

- stream 升级为 7，增加 control 1 的 `OXVC` 帧；TS/Zod、Rust、生成 schema、fixture、
  probe 和当前规格同步升级，拒绝旧版本，不迁移或删除恢复数据。
- 控制句柄绑定唯一 helper 生命周期，容量 8 的队列与音频共享 IO worker。每个音频块间
  最多执行一条控制命令，所有序列与回复身份均核对。参数检查 ID、可写性和归一化范围。
- `openEditor` / `closeEditor` / `poll` / `setParameter` / `capture` 在 helper 主线程串行。
  窗口改为可泵送对象，配置模式保留 Apply/Cancel，live 模式只有 Close；detach 厂商 view
  后才关闭父窗口。窗口关闭不删除处理实例，再次打开复用该实例。
- setParameter 与 GUI 修改经真正零长度总线缓冲进入 processor。仅将 block_size 设为 0
  不足以让底层库处理零帧；时钟 fixture 捕获了这个错误，现已使用专用空声道缓冲。
- capture 在 process 调用之间取得 component/controller state，不调用 setProcessing 重置。
  参数失效通知在有界零样本循环内同步，完整参数表按 ParamID 比对；先清空队列再同步批量值，
  避免与下一音频包的自动化竞争 4096 项队列。动态 I/O/latency/参数表、reload 或重启风暴
  明确使旧流失效，不在旧图中更改 PDC。
- 控制总截止时间覆盖排队、IO 与回复；超时中断 socket、使 Session 失效并回收 helper。
  包括返回调用者前的截止检查，避免线程调度延误造成迟到的成功。close 中断正在等待的控制，
  旧句柄不能操作新建 Session。音频 callback 不增加分配、锁、IO 或 JS 调用。

state 生命周期依据 [Steinberg Processing FAQ](https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Processing.html)，
并通过持续计数的真实 VST3 processor 验证。

## 验证

- `cargo test --workspace`：619 passed，3 ignored，87 suites；host+stream 专项最终 58 passed。
- `pnpm test`：463 passed。`pnpm format:check`、`pnpm lint`、`pnpm typecheck`、
  `cargo fmt --all --check`、fixture fmt、`git diff --check` 通过。
- `pnpm build`、`pnpm build:wasm`、`pnpm schemas`、packed SDK/helper smoke 通过。
- 实际 VestiGain：每种模式 1200 个 PCM 块、60 次参数修改、30 次 state 捕获，增益结果正确；
  将捕获状态恢复到新 helper 后 PCM 一致。窗口模式反复 attach/detach/reopen，以及窗口仍打开
  时关闭 Session，均完成；所有旧句柄拒绝，进程被回收。
- transport fixture：400 块、26129 帧，间插 state 捕获，DSP 计数、音乐时间、seek/loop reset
  和可变帧数连续。配置 fixture 额外验证 live bulk values 进入 DSP，以及 I/O/reload/重启风暴
  使旧流失效；这些测试已由现有 macOS CI conformance 入口执行。
- 工程回归：增益、mix、旁路、串联、Master、state、frame 73 自动化正确；模拟 sink 的
  1058 块中 xruns、deadline misses、NaN、queue drops、plugin faults 全为 0。

## 性能证据

Apple M4 / macOS Darwin 27.0.0 / arm64，48 kHz，128 帧，release。控制 probe 使用离线
PCM 端口，前三个 cycle 预热，保留 54 次参数命令与 1080 次音频往返。测量期间没有并行运行
本任务的编译或测试；机器其他工作负载不受此任务控制。

| 模式     | 参数命令 p95 / p99 | 音频往返 p95 / p99 |
| -------- | ------------------ | ------------------ |
| 无窗口   | 0.119 / 0.142 ms   | 0.158 / 0.255 ms   |
| 原生窗口 | 0.185 / 6.622 ms   | 0.206 / 0.442 ms   |

这些是控制/隔离往返耗时，不是设备 callback 指标。窗口创建和厂商主线程工作可能阻塞 helper；
本轮未证明连续设备播放中打开/拖动窗口不会 underrun。手势的人工拖拽、缩放、Apply/Cancel
视觉验收仍未完成。历史 insert_automation 基线差异未因此清除。

原始记录：

- `benchmarks/results/2026-09-20-vst3-live-control.json`
- `benchmarks/results/2026-09-20-vst3-live-editor.json`
- `benchmarks/results/2026-09-20-vst3-control-transport.json`
- `benchmarks/results/2026-09-20-vst3-control-configuration.json`
- `benchmarks/results/2026-09-20-vst3-control-project.json`

## 继续推进

将控制句柄按稳定 instanceId 和 accepted graph generation 接到 N-API/Preview，再迁移 DAW
入口与 Node Document Service 的源码事务；补充 begin/perform/end 手势及录制、窗口打开时
的连续模拟播放验证。随后处理动态图协商、多输入/效果多输出、MIDI 输出与 surround。
商业资源/负载矩阵、Intel 实机、窗口人工验收和发布签名/公证仍是独立未完成项。
