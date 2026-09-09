# Pattern 声部、Playlist Automation 与内部窗口

本次继续实现 Channel/Pattern/Track 分工与统一 GPUI 内部窗口。修改基于已有未提交工作区；
本记录只描述本次增量，不代表完整 DAW 交付矩阵已经关闭。

## 行为

- Pattern parts 由 Channel 与独立 leaf Pattern 组成。编译、重复播放和音符编辑各自保留声部；
  composite 钢琴窗通过 Channel 标签选择 leaf，编辑作用范围显示为 Pattern part。
- Track 可重叠放置 Pattern、Sample 和 Automation。Browser 使用紧凑资源行，拖动有落点预览，
  释放时验证实际 Track、滚动视口、遮挡与稳定 clip 身份。Undo/Redo/Save 写回真实源码。
- Playlist Automation 以 clip 本地时间求值，重叠时较晚起点优先，覆盖结束后底层片段恢复原相位；
  无 active lane 时恢复静态参数。最后一个 clip 删除后 lane 仍为 playlist 模式。
  片段显示所属参数和曲线缩略图；预览缓存与 accepted snapshot 同步，不进入音频回调。
- Browser、Piano、Mixer、Automation、Plugin Library 和插件面板使用同一 GPUI WindowManager。
  公共标题栏、置前、移动、八方向缩放、最大化/恢复、关闭、Piano/Mixer 停靠共用实现。
  主传输栏始终位于 desktop 外；关闭插件视图不影响 DSP。
- 宿主缩放夹紧可见窗口，恢复时保留用户几何。插件初次大小按 manifest 并留出移动空间；
  窄插件库折叠分类、上下排列详情，钢琴与 Automation 工具栏可横向滚动。
  钢琴视口按音乐位置保持中心，避免改变面板高度后音符消失。
- engine wire 升级到 1.2；TS/Rust 同步验证 parts、playback、automationClips 的版本下限。

## 工程边界

窗口几何、公共框架、内容组合、手势、Playlist 绘制、钢琴状态和曲线预览分别拆分。
格式化后 WindowManager 226 行、框架 197 行、组合 141 行、Playlist clips 250 行、
钢琴状态 150 行、Automation thumbnail 116 行。规范见 `../docs/22-engineering.md`。
`Project` 公共 facade 保留 322 行，涵盖公开 builder/委托方法；Pattern 注册和 Automation
存储已独立，不再向 facade 追加执行算法。保留这一窄例外避免人为拆散公开类入口。

## 验证

- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`git diff --check` 通过。
- `cargo test --workspace` 通过；最后的 UI 曲线缩略图集成后 Preview 回归为 44 passed、2 ignored。
  两个 ignored 是显式运行的性能测试。核心 172、协议 39、CLI 70 个测试通过。
- Rust Playlist 回归覆盖独立声部、移动/重复、自动化本地时间/重叠/空档、禁用 Track、
  最后片段删除、真实离线 PCM、零分配及 64/128/256 帧块大小一致性。
- CLI 回归覆盖选定声部编辑、Automation place/move/remove、Undo/Redo/Save 与新进程重开。
- `OXITONE_PREVIEW_CAPTURE_PATTERNS=1 OXITONE_PREVIEW_CAPTURE_WINDOWS=1 node scripts/smoke-daw.mjs`
  在默认窗口与 1060×720 通过：真实标题移动、边角缩放、最大化/恢复、关闭、资源拖放和保存，
  六类内部视图共存时系统窗口数量严格为 1。插件 watch smoke 验证有效 revision 更新同一面板。
- `examples/offline/src/pattern-parts.ts` 可直接通过 DAW 打开；实际截图确认独立声部和
  Automation 缩略图。截图保存在 `target/pattern-parts.png` 与 `target/daw-patterns.png`。
- 所有音频验证使用离线或模拟输出，没有打开系统音频设备。

聚焦基准：`cargo bench -p oxitone-bench --bench playlist -- --sample-size 10 --warm-up-time 1 --measurement-time 1`。
Apple M4、48 kHz、128 帧、离线 DSP：

| Placements | Criterion mean | sampled p95 | sampled p99 |
| --- | --- | --- | --- |
| 32 | 116.58 µs | 128.292 µs | 191.542 µs |
| 1024 | 114.85 µs | 140.917 µs | 215.916 µs |

这是 DSP 块处理分布，不是物理设备 callback 测量；设备与 xrun 不适用。
日志：`target/playlist-bench.log`、`target/playlist-workspace-tests.log`、
`target/internal-window-tests.log`、`target/playlist-cli-tests.log`、
`target/internal-windows-small-final.log`、`target/embedded-plugin-watch.log`。

## 当前边界

Pattern parts 限制为等长 leaf，不支持嵌套 composite；钢琴编辑修改 leaf 定义并影响它的所有引用。
MIDI 按编排 Track 合并声部，沿用 Track MIDI channel。插件详情仍按 owner/slot 寻址并呈现
source 配置，不能宣称旋钮已具有连续实时参数写入；配置修改仍经 Document 事务。
Clip 裁切/绘制、Sample 完整编辑、嵌套 Pattern 和完整插件参数试听仍需后续实现。
