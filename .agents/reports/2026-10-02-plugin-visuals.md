# 插件图形与参数交互

## 交付

- 波表支持 12 层 source 切片、当前周期填充和高亮、A/B 配色、phase/voice 读数；
  Sub 幅度跟随 level。LFO 与滤波图扩大并使用同色填充。
- 效果器共用更大的坐标区、主/次曲线和色彩图例。EQ 四个频段的曲线、节点、分组与
  控件同色，宽窗四组并排，窄窗折行；旋钮刻度、值标签保持主题语义。
- EQ frequency/gain、Filter cutoff/resonance、声明式 filterResponse 与 ADSR 提供直接图形编辑。
  二维手势同时投影两参数，通过一次 Configuration parameters 事务提交；保留实例身份、
  pending 投影、Undo/Redo、Shift 精调、双击默认值、Escape 取消和无操作点击语义。
- 原有开窗流程在 Preview entity update 中读取同一 entity，引发 GPUI 借用 panic；
  recording 初值改为从当前 owner 状态复制到新面板，后续仍通过 observe 同步。
- 图形数学、绘制、节点交互、包络与文档提交分别维护；新增/修改模块均低于 300 行。
  已有 UI 1.0 与音频协议保持不变，中英文插件界面文档同步更新。

## 验证

- `pnpm format:check`、`pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check` 通过。
- `cargo test --workspace`：662 passed、3 ignored；最终 Preview 定向测试：81 passed、3 ignored。
- `scripts/smoke-builtin-panels.mjs`：全部 30 个内置插件开窗通过。
- 浅色 EQ/Filter/Compressor/Wavetable 与 440×540 窄窗 EQ/Filter/Wavetable 捕获通过；
  检查深浅主题的波表、EQ 与 ADSR 图像，校正四列 EQ 控件排版和 hover 节点文字对比度。
- 既有 `--edit-synth` 的波形选择、bank 条件显示、position 拖动、Undo/Redo、取消与保存重开通过。
- 既有 `--edit` 的旋钮拖动、host Mix/bypass、Undo/Redo、取消、无操作点击与保存重开通过。
- `--edit-graph`：EQ、Filter、Wavetable ADSR 原生指针测试通过，包含 XY 投影、单 revision、
  实例隔离、整体 Undo/Redo、Escape、无操作点击、默认值恢复、Save 与文档重新打开。
  NSEvent 测试适配器现保留 click count 与修饰键，避免将双击/精调错误地降成普通单击。
- Release `benchmark_builtin_plots`：Apple M5 Max / Mac17,14 / 48 kHz，26 个效果器图形
  每批构建 p95 **123.084 μs**、p99 **128.041 μs**（100 warmup / 1000 samples）。
  这是控制侧模型基准，无设备、block size、callback latency 或 xrun 测量。

开发 bundle：`target/debug/Oxitone Preview.app`。原生截图及日志：`target/builtin-panels/`。
截图与 NSEvent 验证使用模拟输出，没有打开系统音频设备；不等于物理鼠标或 VoiceOver 验收。
图形只表达 source/default/手势投影，不声称新增实时频谱、gain reduction 或 effective 参数遥测。
