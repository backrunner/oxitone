# VST3 SDK 调度与 IPC 迟到修复

在不增加固定缓冲延迟的前提下，修正常驻 VST3 通道的线程就绪、唤醒与分散写入问题。
本次仍是 SDK 原生底层；没有接入 Engine 音频图、DAW 实时插槽或厂商 editor。

## 诊断与修改

此前两块和八块缓冲的 DeadlineMissed 记录保留在
[生命周期报告](2026-09-19-vst3-managed-lifecycle.md) 与对应 JSON 中。
机器负载回落后，仅加计时的旧实现已能通过；16/32 个普通 CPU 忙线程也没有重现同样的
长停顿。因此不能声称已经证明旧失败的唯一成因，或所有高负载情况均已解决。

代码检查确认并修正以下缺口：

- IO worker 与 helper 原先都是普通调度线程。现在两者分别在就绪前申请 macOS
  time-constraint，period/constraint 为一个 block 周期，computation 为半周期。
  申请结果明确报告，失败保留普通策略；获准不等于后续永不被内核降级。
- Session 原先创建 IO 线程即返回。现在等待其调度与等待器初始化完成，才交出端口；
  等待共享原握手启动 deadline，失败关闭 socket、终止并回收 helper。
- IO 空队列从普通 sleep 改为 100 µs Mach absolute wait，空闲 waitpid 检查降至每
  10 ms；有在途请求时仍立即发现 EOF。音频调用不新增任何唤醒 syscall 或等待。
- 最大事件块原先分成头、PCM 和 256 个事件逐次写入，现在整帧一个 write_all；读取
  为头和整段有界 payload。内核短读/短写仍走同一总 deadline，最后一次完整 IO 返回后
  也检查期限，防止线程延迟恢复后把过期响应当成功。
- 控制侧可读每个实例的合法完成块数、最大空闲等待/发送/响应等待/helper 处理耗时。
  helper 处理时间是墙钟，包含被暂停的时间；空闲等待不是精确的单块排队时间。
  所有计时/统计均在 IO/helper 线程，实时端口、调度器和换入路径保持零分配/释放。

stream protocol 升为 2：Ready 必须报告 helperTimeConstraint；二进制响应头末尾为
processingMicros（向上取整 µs，饱和 u32），请求必须为零。旧版本和未知版本直接拒绝。
TS、Rust、schema、fake helper 和 smoke 调用同步迁移；manager/scheduleVersion 仍为 1。
未增加 loader 或系统音频设备依赖，客户端仍可只启用 stream feature。

## 无设备基准

Apple M4 / Darwin 27 / Rust 1.98.1，release，VestiGain bypass=1，48 kHz / 128 帧 / stereo。
manager capacity=2、queueDepth=4、latencyBlocks=2（256 帧，5.33 ms），每周期拆成
17+111 帧。每个实例处理 40 块，前两个 epoch 预热，32 次激活、31 次回收。
输入为 epoch 相关的确定性信号，逐帧检查延迟、声道与旧实例 PCM 隔离。

压力测试由 probe 自行创建 32 个普通 CPU 忙线程，在成功/异常退出时停止并 join，
每条线程最长 45 秒自停。它模拟 CPU 竞争，不能代替原先未记录完整状态的后台负载。
调用方仍是普通线程，记录实际 caller interval，不补拉错过的周期；设备、callback p95/p99、
CPU 占用率与 xruns 未测，均为 null。

修复前的计时基线分别保存在
[无附加负载](../../benchmarks/results/2026-09-19-vst3-timing-before.json)、
[16 线程](../../benchmarks/results/2026-09-19-vst3-load-before.json) 和
[32 线程](../../benchmarks/results/2026-09-19-vst3-load32-before.json)。
三次均通过，不能把它们写成失败来夸大修复效果。32 线程基线最大空闲等待为
1.998083 ms，最大响应等待为 0.222917 ms，process p99 为 0.003917 ms。

修复后的[无附加负载](../../benchmarks/results/2026-09-19-vst3-deadline-after.json)与
[32 线程压力](../../benchmarks/results/2026-09-19-vst3-load32-after.json)均完成 32 个 epoch、
163,840 帧逐帧校验、31 次旧实例回收；全部 32 个 helper 已退出，无 DeadlineMissed。
每个实例的 IO/helper time-constraint 申请均获准，固定延迟保持两块。

32 线程的单次前后观测如下（ms）：

| 指标                 |   修复前 |    修复后 |
| -------------------- | -------: | --------: |
| 最大 IO 空闲等待     | 1.998083 |  0.200375 |
| 最大发送             | 0.131792 |  0.068625 |
| 最大响应等待         | 0.222917 |  0.244667 |
| 最大 helper 处理墙钟 | 0.030000 |  0.184000 |
| 音频侧 process p99   | 0.003917 |  0.003291 |
| 最大 caller interval | 4.277333 | 18.899833 |

空闲唤醒的尾部延迟降低，其他最大值并非全部改善。这是同机的有限样本，环境负载会变化，
不能据此承诺任意负载下无迟到；尤其普通调用线程的长间隔不能当作真实设备 callback 合格。
无附加忙线程的最终运行也有外部调度抖动：caller interval 最大 16.089125 ms，process
墙钟最大 3.655375 ms。该 probe 未提升调用线程优先级，这些时间包含抢占；即使固定
帧延迟的 PCM 检查通过，也不代表 2.667 ms 设备 callback 预算已经通过。

独立[原始端口基准](../../benchmarks/results/2026-09-19-vst3-stream-v2.json)通过 2,208 块
有序往返、短块、事件与两次进程重建，roundtrip p95/p99 为 0.168500/0.277500 ms。
[固定调度基准](../../benchmarks/results/2026-09-19-vst3-schedule-v2.json)通过两次 epoch、
281,600 帧，deadlineMisses=0，process p95/p99 为 0.003833/0.025708 ms。

## 自动检查

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `cargo test --workspace`：558 通过、3 个基准忽略。仅保留既有 napi dead-code 和
  block/proc-macro-error2 future-incompatibility 警告。
- `cargo test -p oxitone-vst3-host --features host,stream`：41 通过，无新增警告。
- protocol / vst3 TypeScript 测试：56 通过。schema 已由 `pnpm schemas` 重新生成。
- `cargo check -p oxitone-vst3-host --no-default-features --features stream` 通过。
- 新增最大 4096 帧/256 事件批量读写、响应计时/请求清零、超大和截断 payload 检查；
  错序/NaN/挂起/缓慢滴流不计入合法完成数。旧协议与未知协议均拒绝。
- 既有零分配/释放、跨 epoch PCM 隔离、初始化竞态、崩溃、关闭中断和进程回收全部通过。

所有验证均未打开系统音频输出。Engine/PDC、绝对 seek/loop、厂商兼容与 GUI 接入仍是
后续阶段；不能用这份报告替代真实设备或完整工程压力验收。
