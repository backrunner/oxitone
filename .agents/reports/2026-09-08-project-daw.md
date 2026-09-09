# 2026-09-08 工程 DAW 实施记录

已验证完整工程 Document Service：跨文件缓冲区、原生候选验证、共享/单 placement 音符
操作、明确的拆散候选/确认、Save/Undo/Redo、外部 editor Unix IPC、磁盘冲突显示与解决。
音乐回写仅落在项目源码；不插入显式 entity ID，不修改 node_modules。

Automation source 的 24 个入口均可派生 hard/fade replaceRange，TS/Rust golden 对拍，
64/128/256 sample parity，实时 evaluator 零分配/释放。GPUI 实际绘制、候选接受、TS Save
与新进程重开通过；仅选中 lane source 改变。source loop 每轮重复，lane/timeline 覆盖待实现。

插件目录包含全部 20 个 builtin 和静态 npm 元数据、准确版本、平台/hash/签名策略，
选中外部插件可在独立 helper 验证。纯配置派生、单使用/共享参数、insert mix/bypass、
串联 rack review/确认与 literal 后续编辑通过测试。真实 cc fixture.gain + JS-only npm
包完成 GPUI 拆散→数字输入→候选 native 图→Save→新进程重开，其他实例不变，包与库
hash 不变。当前仍 ABI 1，未实现外部 resources/state ABI 2。

GPUI capture 使用 simulated sink；查看 `target/daw-automation.png` 与
`target/daw-configuration.png`，图像与日志属于忽略的本地产物。

检查记录：pnpm lint/typecheck 通过；core 169 tests、protocol 39 tests、CLI 60 tests
通过；cargo test --workspace 通过。后续修改需继续完成相应检查，不用这些数字代表
全部设计 P0–P5 验收。

性能环境：Apple M4 / Darwin 27.0.0 / Node 26.5.0，48 kHz / 128 frames，100 notes、
2 placements、20 catalog entries；1 次 warmup + 10 次实测，p95/p99 均为此小样本最大值。
source-daw benchmark 验证后接受的真实代码事务，Save 每次都有 dirty 内容：

| 操作 | p50 ms | p95/p99 ms |
| --- | ---: | ---: |
| 音符提交 | 216.70 | 280.51 |
| 插件参数提交 | 199.80 | 209.26 |
| rack 候选验证 | 195.76 | 210.29 |
| dirty journal Save | 62.60 | 67.35 |
| 效果器重排 | 532.33 | 884.18 |
| 文本/插件投影序列化 | 1.73 | 4.65 |
| 新进程重开 | 457.28 | 655.66 |

上述为实例迁移后的独立重跑 `target/daw-instances-source-bench-serial.json`，此前并行测试
重叠样本另保留 `target/daw-instances-source-bench.json`。机器有外部负载，重排/重开性能
仍未达标，不能只选择早期更快的数据。native typed insert 编译约 45.118 µs、128-frame
离线处理约 174.77 µs，见 `target/daw-instances-bench-serial.log`，不是设备 callback 指标。

engine 1.1 显式支持 instanceId 与 plugin/effectHost scoped automation；1.0 未升级语义的
编码往返不变，Project.fromSnapshot 显式迁移实例。重排后自动化和 GPUI 选择仍指向原实例，
源码在既有 bindings 之后输出本地 orderEffects 的位置排列，不写 execution ID。真实 npm
C 插件完成参数/拆散/重排/保存重开，包与动态库 hash 不变。

VS Code 扩展已提供 linked sources、真实版本校验、Save/auto save 通路、项目 Undo/Redo、
冲突 diff 和断线重连。最低版本 1.96.4 真实 Extension Host 复现并绕过 bulk-edit minimization
丢弃版本条件的问题；此前两个竞争操作都成功，修复后旧操作正确拒绝。VSIX 打包包含单一
控制 JS bundle，无 native engine；本地安装产物在 `target/oxitone-vscode.vsix`，未发布。
普通 `file:` tab 仍只有 disk watch，完整 npm TypeScript language service 和窗口重载恢复的
端到端验收继续追踪。

本轮检查日志为 `target/daw-vscode-{lint,typecheck,rust,format}.log`、
`target/daw-vscode-cli.log`（60 tests）、`target/daw-vscode-editor-test.log`（含关闭时立即
取消 pending flush 与非法 conflict choice）、`target/daw-vscode-host.log`。真实 host
覆盖后台 tab、auto save 和 production ProcessExecution → CLI → headless simulated GPUI，
不只连接测试 server。打包记录 `target/daw-vscode-package.log`，产物未提交。

本轮 required focused benchmark 记录 `target/daw-vscode-source-bench.json`，同环境、1+10
样本，与本轮 lint/打包有部分重叠：音符 p50/p95 183.18/206.27 ms，参数 186.62/411.35 ms，
rack 168.34/181.42 ms，dirty Save 58.54/66.71 ms，重排 164.34/173.31 ms，投影
1.26/1.35 ms，重开 165.25/173.10 ms。保留此前较慢样本，不视为大工程性能门禁已通过；
本次没有改 callback，没有新增硬件 callback p95/p99/xrun 证据。

原生 automation range Criterion 测试见 `/tmp/oxitone-auto-range-bench.log`：375 次 control
evaluate 与 48,000 samples 的 audio evaluate，含 hard/fade。control fade 点估计 87.244 µs，
audio hard 8.2142 ms、fade 12.394 ms。没有测设备 callback p95/p99 或 xrun；上述控制/
离线工作量不能冒充真实输出设备延迟。

剩余完整门禁仍包括完整 Definition resolver、GPUI 替换/添加、Arrangement/窗口与新随机域、
lane ranges、Sample/Slicer/regions、ABI 2 配置/资源/状态/UI、安装/更新/卸载/修复、增量
参数试听/实例移交/active generation、完整 IDE language service、新文件/资产保存与大工程性能。
实施目标和验收矩阵保持完整，不以本次闭环覆盖尚未完成的功能。
