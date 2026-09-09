# 钢琴窗力度绘制、延展网格与浮窗圆角

本轮完成 FL 风格的绝对力度绘制、力度不透明度、跨越原长度的钢琴编辑，以及内部窗口
统一圆角。契约与使用说明见 [DAW 控件](../docs/23-daw-controls.md) 和
[Preview](../../docs/preview.md)。

## 行为

- Velocity 区直接点击设值、横向连续绘制；快速移动在事件间插值，不漏掉途经的起点。
  同起点和弦成员共用命中范围，不受时长差异影响；有选择时只绘制选中的成员。
  低力度音符更透明，零力度仍可命中。状态栏显示当前位置的 0–127 力度。
- 实际音符与力度柄同步更新，释放提交一条文档事务，等待接受时保留图形；Escape、
  Undo/Redo 与选区恢复延续原协议。力度绘制不改下一笔音符的默认时长，也不自动把
  无选择的画笔限制到刚修改的子集。删除仍只在音符区进行。
- 横向网格根据滚动位置增加虚拟范围，只绘制当前视口。越界插入、移动、复制和延长
  音符会经 Document 同时延长源 Pattern，末尾向上取整到整数 beat。缩放与滚动在
  接受长度更新后保持；短乐句从停靠区变成浮窗时保持左端可见，避免居中计算藏掉音符。
- `.edit(operations, { lengthBeats })` 显式保存容器长度；上游 chord/arp/repeat 规则
  保持，后续对结果 repeat 使用新周期。literal 与本地拆散同步写回长度，import/export
  保持，npm 代码不变。已有未变的长尾、删除和纯力度/音高改动不触发延长。
- Composite root 容纳最长声部，短声部不提前循环。调度、MIDI 和钢琴播放指针共用
  root 周期；wire 边界用精确有理数比较。自然长度和 loopCount 随 Pattern 更新；
  显式 Playlist duration/lastBeat 保持原裁切边界。
- 内部窗口共用 8 px 圆角，标题栏内半径 7 px。GPUI 内容 mask 为矩形，因此底部保留
  7 px 壳体边距，Canvas、滚动内容与 Mixer 尺寸不覆盖圆角。八方向缩放继续可用。

## 工程与性能

力度算法、音符提交/选区恢复与指针手势分开，`note_edit.rs` 为 237 行，
`note_velocity.rs` 为 113 行，`note_commit.rs` 为 123 行。删除旧相对力度拖动计算。
源码长度推导独立于已有 ProjectDocument 文档所有权/事务状态，未重构无关文档服务。
每次力度移动直接借用已接受的输出；选区恢复使用音乐值索引与重复值队列，避免平方扫描。
这些工作全部在 UI/控制线程，音频 callback 没有新增分配、锁或 JavaScript。

[基准数据](../../benchmarks/results/2026-09-09-piano-workflow.json)：Apple M4、macOS 27，
release 构建，100 次预热、1000 次采样；其他测试、窗口捕获与构建完成后测量。
10,000 音符下，力度笔画 p95 105.625 µs、p99 111.125 µs；完整选区恢复 p95
898.542 µs、p99 914.791 µs。这里测量的是算法耗时，不是 GPU 帧、源码事务往返或
物理设备 callback，sample rate/block size/xrun 不适用且记录为 null。

## 验证

- `pnpm lint`、`pnpm typecheck`、schema 生成、`cargo fmt --all --check`、`git diff --check` 通过。
- Core 179、Protocol 40、CLI 97 测试通过；补充 npm 延长、复合声部保存与 writer
  长度归约后，相关 CLI 5 个用例再次通过。初次新增 Core 测试错误地按输出下标追踪
  移动后的音符，已改为音乐 selector 并重跑完整 Core，未改变排序语义。
- `cargo test --workspace`：537 passed、2 ignored。最后的有理数边界修正另通过
  Playlist 4 个用例（含极接近的 wire 长度）和 Preview 60 passed、2 ignored。
  MIDI 回归验证短声部在 root 周期后的正确 tick 重复。
- `node scripts/smoke-ui.mjs`：13/13 通过，覆盖深浅主题、大小视口、内部窗口、
  插件、Track/片段拖动、自动化、保存与关闭。所有音频使用 simulated sink。
  钢琴原生事件回归验证力度扫绘、等待投影、越界插入、长度 Undo/Redo、Save 与
  全新工程重开。NSEvent 测试构造器的右键 buttonNumber 修正后，明确断言右键编号
  与窗口坐标，避免把左键事件或错误位置当成右键回归。
- 浮窗回归发现短乐句缩小视口后被居中滚动移出屏幕；已增加左侧锚点回归，控件与
  窗口场景随后通过。两套主题的 Browser/Piano/Mixer 圆角已目视检查。系统截图有时
  保留较早的 compositor 帧，音乐最终值以 accepted snapshot 与保存重开断言为准。
- 最后的选区索引优化后，另跑浅色 1060×720 钢琴原生冒烟并保存重开，通过。
  [钢琴截图](../../target/piano-final-light.png) 显示低/高力度不透明度与第 21 拍音符；
  圆角见 [浅色浮窗](../../target/ui-review/windows-light-small.png) 与
  [深色浮窗](../../target/ui-review/windows-dark.png)。

最新 unsigned 开发 app 和 native facade 已重新构建；未提交二进制或其他构建产物。
