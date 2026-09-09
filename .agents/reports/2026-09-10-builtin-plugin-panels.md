# 内置效果器与音源面板

范围：26 个 bundled 效果器、Sampler/Multisampler/Slicer 和已有 Wavetable 专用面板。
每个插件都有按音色处理职责划分的控件、可读模式选择与原生向量可视化。
图形矩阵及精确语义见 [11-plugin-ui](../docs/11-plugin-ui.md)。

## 实现与边界

- EQ 使用 DSP 的 biquad 系数，零增益居中；Filter 使用效果器的真实 Q，避免误用合成器的
  normalized resonance。非线性滤波显示小信号响应。动态处理器显示静态曲线和时序示意。
- Delay 显示回声间距与反馈衰减，并按显式秒数/默认拍数的实际优先级选择时间控件。
  调制类显示左右轨迹；整形器显示传递函数；音源显示键位/力度区域、slice 触发与包络。
- 图形缓存于 UI Entity，参数手势只重建相关插件的模型；主题、运输游标及其他窗口手势
  不重复计算这些曲线。绘制不创建 DSP 实例、不读取音频指针、不调用 process。
- 面板共享 knob/fader/toggle/choice 编辑入口；Shift 精调、双击恢复旋钮/fader 默认值、
  Escape 取消。窗口捕获跟踪跨控件拖动，鼠标释放提交一次实例配置事务。
  点击但不拖动不会因浮点归一化误差生成修改。右键释放不提交左键参数手势。
- 通过实例身份保护旧事件，接受的工程变化取消未提交手势；图形投影一直保留到新快照
  接受或事务被拒绝。源码错误/构建阶段正确刷新控件的可编辑状态。
- Host Mix/bypass 与 plugin 同名参数分离。沿用 Document Service 的保存、撤销、重做、
  import 生成和局部配置派生，不在 GUI 写 TS 或 node_modules。
- 第三方显式注册布局继续优先，使用相同控制事务；普通 Preview 保持只读。
  宿主专用曲线只匹配 bundled 的准确版本，不给外部同名插件假定算法。
- 内部窗口宽度变化会通知插件重新布局。按分组安排初始尺寸，双图/双组使用适合宽度，
  窄窗纵向滚动；轴文字按实际文字宽度夹紧，控制组去掉冗余参数前缀与空工具行。

## 验证入口

- `pnpm lint`、`pnpm typecheck`、`cargo fmt --all --check`、`cargo test --workspace`。
- `plugin_plot_tests` 验证全部 30 个插件的 descriptor 绑定覆盖、图形存在、效果器参数极值
  的有限/有界工作量、Q/压缩比语义、时间参数优先级、极端频移与 enum/log 映射。
- `scripts/smoke-builtin-panels.mjs` 使用临时项目和 simulated sink，覆盖逐个开窗、两套主题、
  窄窗，以及真实 NSEvent 参数拖动、投影、实例隔离、Undo/Redo、Escape、Mix/bypass、
  保存重开和无操作点击。日志与图像在 `target/builtin-panels/`。
- `scripts/smoke-preview-panels.mjs` 验证真实第三方 dylib 布局、watch、坏源码/布局回退和恢复。
- `benchmark_builtin_plots` 测全体 26 个效果器图形模型的 release 构建耗时；结果单独存档。

## 验证结果与限制

本次结果：lint/typecheck/fmt/diff 检查通过；workspace Rust 测试 **542 passed、0 failed、
3 ignored**。全部 30 个插件逐个开窗通过，另外通过 6 个浅色、6 个浅色窄窗场景及两套主题
的参数编辑保存重开。最终图像逐项检查，取得旧 compositor 帧的场景重试后确认面板可见。
真实第三方 dylib watch 的语法/runtime/native/layout 失败与恢复全部通过。
Debug 与 release 开发 app bundle 均已通过构建，executable 使用临时副本加原子 rename 更新；
未提交构建产物，未进行签名发布。

Apple M4 / macOS 27.0 / 48 kHz 下，全体 26 个效果器图形模型每批构建 **p95 173.625 µs、
p99 272.583 µs**（100 warmup / 1000 samples）。这是控制侧模型基准，
详见 [结果](../../../benchmarks/results/2026-09-10-builtin-panels.json)，不作音频 callback 性能声明。

曲线使用 source/default 参数，不是频谱、实际 IR 波形、gain reduction 或 effective 参数遥测。
自动 onset 的最终 slice markers 尚未暴露到 ViewProject，面板显示真实声明的检测灵敏度。
物理鼠标、IME、VoiceOver、原生全屏和真实设备 callback 不由定向 NSEvent/截图替代。
锁屏桌面的系统截图偶尔取得之前的 compositor 帧；状态断言通过不等于截图内容正确，
图像需另行检查并重试。整个任务不打开系统音频输出。
