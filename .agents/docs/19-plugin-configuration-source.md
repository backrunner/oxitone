# 插件配置的源代码派生

VST3 参数录制保持当前配置及原自动化，新增独立 Playlist 层。DAW 回写局部
AutomationSource 定义和常规 param.automate/createAutomationClip 调用，原 Project
表达式仅求值一次。每条曲线保留 Source 编辑边界；一次候选验证/接受组成一次 Undo，
失败不写文件，完整 take 可重试接受。Save 不终止同一 revision 的正在进行的录制。
捕获/任务/图身份仅在控制协议中传递，不写进用户源码。完整规则见 [24](24-vst3-sdk.md)。

VST3 使用 Engine 1.3 的 instrument/effect state 和独立 registrationVersion 1。
Assign 会通过既有 configure 源码事务持久化精确 class/hash、helper 路径、归一化参数与
configuration；必要时附加 withVst3Registration。Undo/Redo 同时恢复注册与配置，Save
仍只发布源码候选；旧 C ABI 1 不因此获得 state 能力。Preview snapshot 新增 vst3Plugins，
native viewer 在控制线程重新验证并编译实例，候选失败保留上一个可播放版本。

本增量实现纯值 `pluginConfig(kind, input).withParameters(values)` 与效果器 `.withHost({mix?,bypass?})`。
`.withState(state)` 返回替换状态的冻结派生值，保留其余配置，拒绝不可序列化输入。
`.replaceParameters(values)` 替换完整参数表，供配置变更后移除旧 ParamID；withParameters 仍合并。
`kind` 为 instrument/effect，input 接受现有 builtin helper 或 npm factory 的声明式 ref。
参数按原样字符串键合并，点号不是嵌套路径。构造/派生不加载库、不创建 DSP；数据冻结，
资源与 structured builtin state 保留；不可序列化状态、非有限数与不合法 host 值拒绝。
参数名称/物理范围仍由准确插件的原生 descriptor 在候选 compile 时权威验证。

源代码 capture 只给可能产生对象的值边界分配身份。确定为原始值的字符串、数字、bigint、
布尔和 null literal 不参与 capture，不消耗 4096 个边界预算；它们不能是配置、实例或音乐
对象。包含 4096 个参数的 state/参数 literal 仍按整个配置对象编辑和校验，不人为提高预算。
函数调用、标识符及其他需要运行的表达式继续原样求值与 capture。

Channel/Bus 仅在控制侧保留配置输入对象身份。求值器从受信任 Project 对象关联 capture，
局部 reference 必须被单次执行的 owner 边界包围；同一 owner 的重复共享使用不能猜选。
配置参数编辑不重排或替换实例。projection 采用 engine 1.1 的实例 handles，owner/slot
只描述当前显示位置，不写用户 TS。实例/typed automation 与重排见 [20](20-plugin-instances.md)；
typed runtime commands 与 ABI 2 仍待迁移。

Document `configuration` operation 带 site、可选 usage 和 parameters/host edit。默认局部使用，
定义范围显式共享；完整工程比较保证只改准确配置，其他实例、宿主同名参数、automation、
路由/资源/插件注册不变。失败保留原代码和 accepted graph。相同 wrapper 的重复参数编辑
合并 literal patch；保留函数调用、副作用、非 literal 参数、注释及 import/export。
归约识别 writer 发射的字符串计算属性（如 `["oscA.level"]`），按完整参数 ID 合并，
不执行动态 key；重复 key、`__proto__` 与需要求值的计算属性仍拒绝归约。
writer 复用可见 pluginConfig 别名/namespace，避开 type-only 与遮蔽；普通 Save 仍走统一 journal。

VST3 实例面板的 Open/Close native editor 发出 `vst3.controlInstance(site,usage?,action)`，
只接受单实例配置边界，经 Node 校验后控制 Preview 正在使用的实例，打开/关闭不产生 revision。
Use current state 发出 `vst3.captureInstance`，捕获 class/hash 绑定的 state/参数，再验证 source reads、generation 和 revision，
写成 `pluginConfig(kind, original).replaceParameters(values).withState(state)` 并原生验证候选。
不替换 Project slot，因此实例 ID、typed automation、mix/bypass、其他相同插件保持不变。
重复接受相同配置不产生 revision；状态变化时仅归约 writer 发射的 literal wrapper，保留工厂调用、副作用与注释，不堆积旧
opaque payload。accepted graph、Undo/Redo 和 Save 沿用同一源码事务；厂商窗口修改直接
影响播放实例，显式采用状态前不会改变源码。没有可隔离边界时按钮不可用，可先走现有
源码/链拆散流程。独立克隆的 editInstance 已删除，catalog 的独立配置 editor 仍保留。

GPUI Plugins 是名称/类型列表，不显示参数参考表；Used in project 按需展开使用位置，
Edit 打开独立实例配置窗口。在配置窗口输入物理值（Enter 提交、Escape 取消）、
增减、恢复 default 或修改 insert mix/bypass；明示 initial configuration，与播放中的 effective
值分开。所有操作仍通过 Node 候选验证，不调用实时 setter。DAW 独立面板也可直接拖动
旋钮/fader、选择枚举、切换 toggle 及编辑 host Mix/bypass；默认解析当前实例的 reference
site，复用相同 configuration 事务，释放一次提交，普通 preview 仍只读。
实例配置窗口标题显示归属、插件名称和效果器槽位，独立持有目标和滚动；
点击使用位置 Edit 或从详情导航时重置配置滚动，确保参数立即可见。
浏览插件库不会重新定位已有编辑目标；窗口仅显示编辑控件，不重复源码表达式/使用列表。
Shared 模式保留受影响使用数量。窄窗口的作用范围、重排、拆散及包管理按钮自动换行。

串联数组 factory 的输出另有 rack site。`planMaterializeRack` 构建与原工程完全等价的候选，
review 展示源文件前后全文、每条链的效果器数量、使用范围、失去预设更新关系以及依赖保留。
确认绑定 planId/revision/读集。取消、代码变化、其他事务或 Save 使计划失效；拆散本身不改
缓冲区或文件，直到确认。绑定变量写成普通 EffectRef 数组；调用/getter/await 使用 sequence
保留原表达式恰好一次执行。外部 native 库仍保留，不能把本地化配置当成 DSP 替代。
后续编辑 literal 直接改参数字段，不新增 wrapper。拆散保持数组顺序和现有 automation；
独立重排事务保留 typed binding，旧 index 绑定若会改变所指实例则拒绝。图形删除尚待接入。

测试通过真实 cc 编译的 fixture.gain 动态库及仅 JS 的 npm rack：review/cancel/confirm、
两个相同插件只改一个、其他 Channel/Bus 保持、原工厂调用次数保留、参数越界拒绝、保存
新进程重开和 npm JS/动态库 hash 不变。没有打开音频设备。

这是现行 ABI 1 的初始配置回写，不声称外部资源/state、参数连续试听、typed event targets、
插件安装/替换/升级、并行 rack 和 opaque DSP 拆散已完成。
