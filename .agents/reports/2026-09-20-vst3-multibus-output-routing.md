# VST3 多总线、侧链与工程乐器多输出

本轮完成原生多总线传输、工程 bus insert 侧链和乐器辅助输出到 mixer 的路由。
这消除了 Reaktor 6 乐器因八路输出而无法注册的已知限制；完整 VST3 功能与发布验收仍有下列未完成项。

## 实现

- stream 6 保留最多 16 个 mono/stereo 物理输入/输出总线及 active 状态；40 字节包头
  明确方向总线数，输入/输出可以不对称，中间 inactive slot 不压缩。控制准备显式激活。
  Ready、TS schema、原生校验、PCM codec、预分配队列与恶意 helper 测试已同步。
- 原生 bus port 可完整提交/接收所有总线；旧双声道接口和 ScheduledPort 对多个物理
  总线明确拒绝。无音频输入使用全零占位，mono 输入取均值、输出复制为 stereo。
- Project bus insert 将 mixer detector 送入 VST3 bus 1，沿用 sidechain DAG 与 PDC。
  缺省 detector 为零，支持发送量、host bypass 和 state 保存恢复。
- Engine 1.4 增加 `Channel.outputRoutes`，将辅助输出索引 1…15 映射到 mixer bus。
  输出 0 保留 Channel 插入链；辅助输出绕过插入链，共用轨道 level/pan/mute/solo，
  每路独立 PDC。一个实例每段只处理一次，未路由的辅助输出显式停用；按 Channel ID、
  数值输出索引稳定求和。路由修改通过重新编译和既有 block boundary 换图生效。
- TS options、setter、快照、恢复、生成 schemas 与 Rust compile 一致。恢复测试发现的
  hydrate 遗漏已修复；索引非法、目标不存在、插件不提供该输出、旧版本标注均拒绝，
  失败不改变已接受的工程或插件实例身份。旧格式恢复数据不会被重写。
- Mixer 的 Routing/Signal Flow 和 bus INPUTS 显示各条物理输出。多个输出指向同一
  bus 时保留独立连接和 UI identity；标明输出编号及绕过 Channel inserts 的信号位置。
- 按真实 category 分类，带音频输入的 Instrument 不再被当作 Effect。Reaktor 6 FX
  的八输入/八输出工程路由仍不支持，不能把原生传输能力当成工程图能力。

辅助输出只允许隔离执行器。设备 callback 不调用插件、JS 或 N-API；数据路径使用
预分配缓冲。Rust 测试验证辅助输出、短段、PDC 和复位处理期间 allocation/free 均为零。
构建乐器与编译 Channel plan 分别拆到专属模块，避免继续扩大原编译装配文件。

## 验证与证据

- `cargo test --workspace`：612 passed，3 ignored，86 suites。
- `cargo test -p oxitone-vst3-host --features host,stream`：51 passed，6 suites。
- TypeScript 各包与示例合计 459 passed。首次全包运行的 Web 测试因缺少
  `dist/oxitone.wasm` 失败；完成 `pnpm build:wasm` 后 Web 的 9 项及后续包测试全部通过。
- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、
  `git diff --check` 均通过。workspace、原生 addon、Wasm 和 debug Preview bundle 已构建。
- 实际 VST3 fixture 验证 1600 个包、104100 帧：sidechain、inactive middle、全激活、
  非对称激活、mono 转换、短块、reset 和所有输出 PCM。
- 原有 transport fixture 在新协议与 factory 上再次通过 400 blocks、26129 frames、
  79 次 reset，覆盖 tempo/meter、cycle、短段及插件实际时钟。
- 新 MIDI 乐器 fixture 的三路输出分别为同一声部的 1/2/3 倍；通过真实 SDK/native/helper
  验证单路、求和、fader、mute、取消/切换路由、mixer 与 Track stems，以及保存重开
  后逐样本一致。包含 8 项非零 PCM 比较，最大误差约 3.73e-9。
- 多输出模拟 sink：404 blocks；loop、换图、seek、stop 后 xruns、deadlineMisses、
  nanBlocks、queueDrops、plugin faults 均为 0。后台 block 直方图桶 p95=0.262144 ms，
  p99=0.524288 ms；这些不是硬件 callback 数据。
- 已核对原生自动截图 `target/vst3-output-routing.png`：三个输出卡片、编号、目标及
  Routing 数量正确，无重叠。capture 还验证 inspector 键盘滚动与静音模拟输出模式。
  这不替代厂商 GUI 的人工 Apply/Cancel/resize 或实时参数手势验收。
- 实际 Reaktor 6 Instrument（class `5653544e6952367265616b746f722036`）已成功注册，
  八路输出全部路由并完成离线渲染，图延迟 264 frames。未载入 ensemble，结果静音；
  未验证商业音色、外部资源、听音质量或授权交互。
- 更新后的原生包通过实际 VestiGain 工程测试：bypass/gain/mix/serial/Master/state、
  非块对齐参数事件、模拟 play/seek/loop/pause/stop，xrun/deadline/fault 均为 0。
- 真实 VestiMIDISynth 的 DAW 分配、registration/state 源码回写、Undo/Redo/Save/reopen、
  MIDI note-on/off 和 native WAV 均通过，并生成更新后 Preview 通用乐器面板截图。
- packed VST3 SDK 在仓库外解析并执行对应平台 helper，错误的显式 override 正确失败。
  macOS arm64/x64 CI 已接入多总线、侧链与工程输出测试；未触发远端 CI。

原始数据：

- [总线、输出路由与模拟播放](../../benchmarks/results/2026-09-20-vst3-output-routes.json)
- [VestiGain Engine 1.4 回归](../../benchmarks/results/2026-09-20-vst3-project-engine14.json)
- [原生侧链阶段](../../benchmarks/results/2026-09-20-vst3-multibus.json)

## 性能门禁仍待确认

执行了 `insert_automation` Criterion 基准（48 kHz、128 frames、1 s warmup、3 s
measurement、30 samples；Apple M4、Darwin 27、Rust 1.98.1/LLVM 22.1.8）。第一轮相对
历史记录显著变慢，连未改动的 resolve 路径也约为原来的两倍；当时有其他用户进程负载。
不能据此把差异全部归因于环境，也不能宣称没有回归。

随后运行保留的旧程序，再重跑当前程序。第二轮当前 render_128 / typed render_128
估算为 318.59 / 273.95 µs，对旧程序仍有差异。旧程序的 Cargo rustc 指纹与当前不同，
精确源码版本也未证明，因此这组对照只能用于诊断。未关闭用户的其他程序，未放宽任何
阈值；需要同一编译器、已知源码基线和稳定负载的后续性能复核，发布性能门禁未清除。
此前回归记录与此次所有原始样本均保留：

- [第一轮](../../benchmarks/results/2026-09-20-vst3-output-routes-benchmark.json)
- [旧程序诊断](../../benchmarks/results/2026-09-20-vst3-output-routes-old-binary.json)
- [当前程序重复对照](../../benchmarks/results/2026-09-20-vst3-output-routes-benchmark-repeat.json)

## 未完成项

1. 运行中实例的厂商 GUI 联动、实时参数试听和 begin/perform/end gesture 自动化录制。
   现有 editor 仍是独立静音配置会话，Apply 后源码事务与重编译。
2. 动态 I/O/latency/controller 重协商；当前运行期变化会 fail closed，需显式重编译。
3. 效果器多输出和更多输入的工程路由、MIDI 输出路由；surround 超出当前 stereo 图。
4. 更多商业插件与 ensemble/资源恢复、长时压力矩阵、Intel 实机、厂商窗口人工交互。
5. 上述性能复核，以及发布签名/公证和远端流程验收。本轮未提交、发布或部署。

## 复现

```sh
pnpm build
pnpm build:wasm
node scripts/build-preview.mjs --debug
cargo test --workspace
cargo test -p oxitone-vst3-host --features host,stream
pnpm test
OXITONE_VST3_VIEWER="$PWD/target/debug/Oxitone Preview.app/Contents/MacOS/oxitone-preview" node scripts/smoke-vst3-buses.mjs
OXITONE_VST3_FIXTURE=/path/VestiGain.vst3 node scripts/smoke-vst3-project.mjs
cargo bench -p oxitone-bench --bench insert_automation -- --warm-up-time 1 --measurement-time 3 --sample-size 30
```
