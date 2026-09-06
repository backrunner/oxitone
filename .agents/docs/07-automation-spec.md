# Oxitone Automation 执行规格

本文是 Automation source 的规范性定义。TypeScript builder、wire schema、Rust validator、Rust evaluator、离线渲染和 MIDI export 必须遵循同一语义。

## 1. 坐标与输出

- 输入时间 `t` 使用 Project beat，beat 0 是 Project 时间轴起点。
- 所有周期函数使用 `u = euclid_mod(t - phase, periodBeats) / periodBeats`，所以 `u` 始终在 `[0, 1)`。负 seek 位置无效，但负 phase 合法。
- lane 最终输出必须是有限数且在 `[0, 1]`。中间组合值可以短暂越界，必须在绑定 target 前经 `clamp` 或最终 lane clamp。
- 恰好落在不连续边界的值采用右侧区间，即所有时间区间均为左闭右开。
- target parameter spec 最后把 normalized value 映射为物理值。Automation source 本身不知道 Hz、dB 或枚举值。

## 2. 基本函数

### constant、line 和 curve

- `constant(v)` 在任意 t 输出 v。
- `line(from, to, duration)` 等价于 `curve([{beat: 0, value: from}, {beat: duration, value: to}], 'linear')`；t 小于首点时保持首值，大于等于末点时保持末值。
- `curve` 的 points 必须至少有 1 个，按 beat 严格递增，不允许重复 beat。每个点的 `curve` 描述从该点到下一个点的 interpolation；末点的 curve 被忽略。
- `polyline(points)` 等价于所有 segment 使用 linear 的 curve。
- `step` 保持左点值直到右点；`linear` 为线性插值；`smooth` 使用 `x*x*(3-2*x)`；`exponential` 要求两端值都大于 0，使用 `a * (b/a)^x`；`bezier` 使用单调时间轴 cubic Bezier，并用有界迭代求对应 x 的 y。

### gate

给定 normalized phase `u`：

```text
gate(u) = on,  if u < duty
          off, otherwise
```

`duty=0` 永远 off，`duty=1` 永远 on。默认 `on=1`、`off=0`。`periodBeats > 0`，`duty/on/off` 均在 0..1。

### wave

波形先得到 `raw`（0..1），再输出 `min + raw * (max - min)`：

```text
sine:     raw = 0.5 + 0.5 * sin(2*pi*u)
cos:      raw = 0.5 + 0.5 * cos(2*pi*u)
triangle: raw = 1 - abs(2*u - 1)
saw:      raw = u
ramp:     raw = 1 - u
square:   raw = u < pulseWidth ? 1 : 0
```

因此 phase=0 时 sine=0.5 且上升，cos=1，triangle=0，saw=0，ramp=1。`min/max` 默认 0/1，要求 `0 <= min <= max <= 1`；square 的 `pulseWidth` 默认 0.5 且在 0..1。

## 3. Chance

`chance` 是按 beat 网格计算的 deterministic sample-and-hold。公开 API 的 `frequency` 和 `rate` 都表示每 beat 的决策次数，二者是别名；wire snapshot 统一保存为 `rate`。决策间隔 `interval` 为：

```text
interval = intervalBeats           // 使用 intervalBeats 时
interval = 1 / rate                // 使用 rate 或 frequency 时
decisionIndex = floor((t - origin) / interval)
```

`frequency`、`rate` 与 `intervalBeats` 必须且只能提供一个，且所选值有限并大于 0。`probability` 在 0..1，每个 decisionIndex 通过 versioned `pcg32-v1` 得到 `r in [0,1)`，`r < probability` 时值为 1，否则为 0。不得依赖之前按顺序求值才能算出当前 index，否则 seek 后结果可能不同；实现应使用 `(effectiveSeed, decisionIndex)` 的可跳转/hash-seeded 求值，或等价的 O(1) deterministic 方法。

`effectiveSeed = hash64(projectSeed, laneSeed, canonicalSourcePath, loopIteration?)`。canonicalSourcePath 是 AST 中稳定的 child index 路径，不能使用内存地址或 traversal cache index。

- `randomPhase: 'absolute'`：origin 为 Project beat 0，loopIteration 不参与 seed；seek 到同一 beat 始终得到同一值。
- `randomPhase: 'restart'`：origin 为当前 play/automation-loop 起点，loopIteration 参与 seed；每次 loop 得到新的确定性序列，stop 后重新 play 会从 iteration 0 开始。
- `smoothBeats=0` 使用 sample-and-hold。大于 0 时，从决策点的旧值线性过渡到新值，过渡长度为 `min(smoothBeats, interval)`，然后保持。
- `probability=0` 永远 0，`probability=1` 永远 1，无需消耗 PRNG。

`pcg32-v1` 的 multiplier、increment、seed mixing、整数到 `[0,1)` 的转换和测试向量必须在 Rust/TS protocol fixture 中固定；算法改变需要 protocol major bump 或新的 `randomAlgorithm` ID。

Phase 1 定值（实现见 `packages/protocol/src/pcg32.ts` 与 `crates/core/src/pcg32.rs`，二者必须逐位一致）：

- multiplier = 6364136223846793005，increment = 1442695040888963407（经典 PCG32 固定流），state 为 u64。
- seeding：`state = 0; advance; state = (state + seed) mod 2^64; advance`。
- step：`state = state * multiplier + increment (mod 2^64)`；输出取自 step 前状态 `s`：`x = u32(((s >> 18) ^ s) >> 27)`，`rot = s >> 59`，`out = rotr32(x, rot)`。
- `[0,1)` 转换：`out * 2^-32`。
- `hash64-v1`：各部分编码为 `n:<u64 十进制>` 或 `s:<utf8>`，以 `|` 连接后取 SHA-256 前 8 字节大端为 u64 seed。
- 测试向量：`schemas/fixtures/pcg32-v1.json`（seed 0/42 的前 32 个 u32 输出与前 8 个 float）和 `schemas/fixtures/hash64.json`，TS/Rust 测试都必须逐字节对拍。

## 4. 组合器

```text
map(x,min,max) = min + x*(max-min)
clamp(x,min,max) = max(min, min(x,max))
invert(x) = 1-x
scale(x,k) = x*k
offset(x,a) = x+a
mix(a,b,t) = (1-t)*a+t*b
add(a,b) = a+b
multiply(a,b) = a*b
min(a,b), max(a,b) = ordinary finite comparison
quantize(x,n) = round(x*(n-1))/(n-1)
```

`map` 要求 0 <= min <= max <= 1；`clamp` 默认 min=0/max=1；`mix` 的 t 默认 0.5 且在 0..1；`quantize` 的 n 是大于 1 的整数。除 clamp 外的组合器不隐式截断中间值，最终 lane 输出执行一次 0..1 clamp。

AST 最大深度 64、最大节点数 256。共享 source 在 wire 中默认展开并分别计数；未来如加入引用节点，必须禁止 cycle。

## 5. 求值速率

- target `rate='control'`：每个 block 起点求值；Rust parameter smoother 负责跨 block 平滑。
- target `rate='audio'`：每个 sample 求值。curve/wave phase 必须由绝对 sample frame 映射到 beat，不能反复累加 f32 phase。
- 特例：全局 tempo target（project 的 `tempo` 参数）不逐 block 求值，而是在 compile 期烘焙成分段线性 bpm 表（见 `03-audio-runtime-spec.md`）；其 source 必须 transport-invariant。
- gate、step、square、chance 的不连续点若落在 block 内，compiler 必须生成 segment 边界，使变化落在正确 sample frame。
- tempo change 只改变 beat/sample 映射，不改变 source 在 beat 域的定义。

Insert targets 使用 04 中的统一路径；初始值 → host event → automation 的同帧
优先级对 Channel/Mixer/Master 一致。beat-unit effect 参数在物理映射后按
`seconds = beats * 60 / effectiveBpm` 转换，未显式设置的拍数参数只有在收到
beats automation/host event 后才启用同步。源 evaluator 与既有 golden 不变。

Tempo lane 烘焙必须应用 lane 自身的 loop/lastBeat。loop 内按其 region 周期映射
source beat，结束后保持最终 phase；恰好结束在整周期边界时保持 region 末端值。
每次循环的 source 跳变和 wrap 都加入烘焙边界，hold 端点同样加入；combine 只能 replace。
时间网格及循环边界数量在分配前检查预算，不以越界分配来检测 `TempoMapComplexity`。

## 6. 校验错误码

至少提供以下稳定 code，并附带 source JSON path：

| Code | 条件 |
| --- | --- |
| `AutomationNonFinite` | 任意数值为 NaN/Infinity |
| `AutomationRange` | probability/duty/value/min/max/pulseWidth 越界 |
| `AutomationPeriod` | period/duration/rate/interval 非正 |
| `AutomationChanceFrequency` | frequency/rate/intervalBeats 中不是恰好一个，或所选值非正 |
| `AutomationPoints` | points 为空、beat 不递增或重复 |
| `AutomationExponentialZero` | exponential 端点不大于 0 |
| `AutomationDepthLimit` | AST 深度超过 64 |
| `AutomationNodeLimit` | AST 节点超过 256 |
| `AutomationRateBudget` | audio-rate evaluator 超出 graph 性能预算 |
| `AutomationTempoRestriction` | tempo lane 使用 `chance`/`restart` 语义，或烘焙超出分段预算 |
| `TempoAutomationConflict` | 存在多条 tempo lane 或其 combine 冲突 |

## 7. 必测向量

- gate：duty 0/0.5/1，在 u=0、边界前、边界、周期末和下周期起点。
- wave：六种 waveform 在 u=0、0.25、0.5、0.75、接近 1；验证 sine/cos phase 约定。
- curve：before/at/after points，所有 interpolation，block 和 tempo boundary 连续性。
- chance：probability 0/1、固定 seed 的前 16 个 decision、absolute seek 往返一致、restart loop iteration 不同但可复现。
- composition：嵌套 map/invert/multiply、最终 clamp、depth/node limits。
- parity：相同 serialized source 在 realtime/offline、不同 block size（64/128/256）下，在同一 sample frame 输出一致。
- tempo loop golden（`crates/transport/tests/tempo_lane_loop.rs`）：source 在 beat 0/0.5/1
  为 120/240/60 BPM，loop 长度 1、count 2。beat 0.5/1/1.5/2/3 的累计秒数应为
  0.25/0.375/0.625/0.75/1.75（烘焙容差 1e-5 s），beat 2 以后保持 60 BPM；
  另测 lastBeat、非整周期结束与巨大 horizon 的提前拒绝。
