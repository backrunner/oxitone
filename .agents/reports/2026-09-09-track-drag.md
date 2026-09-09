# Track 快捷控制与直接拖拽（2026-09-09）

按本轮需求增加 Track M/S、移除 Locate，并统一 Playlist 与钢琴窗的直接拖拽显示。
行为契约见 [23-daw-controls](../docs/23-daw-controls.md)，操作说明见 [Preview](../../docs/preview.md)。

## 实现

- Track 名称下提供紧凑 M/S 按钮，色条保留 enabled 控制。Track M/S 独立于共享 Channel，
  对本轨 Pattern、Sample 和 Playlist automation 生效。Mute 优先、多个 Solo 可并存，
  M/S 不改变时间线长度。GPUI/N-API 播放尊重 Solo，WAV 沿用 respectSolo，MIDI 忽略 Solo。
- Engine 1.2 增加 Track mute/solo 字段和低版本门禁。TS setter、快照恢复、Document
  配置事务、相邻字段合并、Rust 调度与导出保持一致；GUI 请求省略未修改的 boolean。
- 删除 Locate 输入、解析器、输入焦点状态和 G 快捷键，保留时间轴、marker 和导航快捷键。
- Playlist 的片段名称、音符缩略图和自动化曲线随本体移动；跨轨移动替换原位置，复制保留
  原片段，Browser 放置与长度调整使用同一 renderer。每帧统一生成 placement 显示列表。
- 松手只提交一次事务；在旧 accepted snapshot 上保留最终位置，匹配的新 snapshot 接替后
  移除本地显示。失败恢复旧状态，避免确认等待期间回弹或复制片段重复出现。
- 钢琴窗绘制和命中检测使用相同 SourceSite 输出顺序，重复音符按输出索引区分。拖动直接
  更新音符和力度显示，鼠标释放位置参与最后一次计算。提交后保存旧输出用于等待显示，
  接受后恢复选择；切换 Pattern 不显示另一 Pattern 的 pending 音符。

所有源码写入经过 Node Document Service，保留 import/export、Undo/Redo 和 Save。
没有向用户 TS 添加 ID，也没有修改 npm 包的音乐代码。播放过滤在控制线程编译阶段完成，
没有向音频 callback 加入分配、锁、JS 或文件操作。

## 验证

- `pnpm lint`、`pnpm typecheck`、schema 生成、`cargo fmt --all --check` 与 diff 检查通过。
- `cargo test --workspace`：528 passed、2 ignored；最终 Preview 回归：53 passed、2 ignored。
- Protocol/Core 测试通过，CLI 完整复跑 95 passed。首轮 CLI 的 atomic-save watcher 测试
  超时；单独复跑和之后完整复跑均通过，不把这次超时记录当作已修复的 watcher 缺陷。
- 新测试覆盖共享 Channel、Mute/Solo 优先级、多 Solo、Sample 与 Automation 过滤、
  静音 PCM、导出 Solo 策略、协议门禁、字段验证、Undo/Redo、Save 后独立重开。
- 13 个串行 GPUI 场景全部通过，包含深/浅主题和 1060×720 小窗口。新增真实 NSEvent
  命中 M/S、跨轨拖动、钢琴窗拖动、松手等待显示、一笔 revision、Undo 和 Save 断言。
  测试的等待断言在原生事件派发后执行；第一次运行暴露了测试提前断言的问题，已调整。
  原生截图用于布局检查；接受和落盘结果由 Document 状态与独立重开断言验证。
- 性能结果见 [track-drag](../../benchmarks/results/2026-09-09-track-drag.json)。UI 投影和离线
  DSP 分别采样，不把算法耗时当作 GPU 帧率、设备 callback 或 xrun 实测。
- `node scripts/build-preview.mjs` 完成 release 构建，通过临时副本加 rename 更新
  `target/release/Oxitone Preview.app`，开发 bundle 未作为发布产物提交。

测试使用 simulated/offline 路径，没有打开物理音频输出。既有局部 Track tempo 的
移动/复制/长度调整限制保持，仍由事务明确拒绝；本轮不宣称完成整个 DAW 能力矩阵。
