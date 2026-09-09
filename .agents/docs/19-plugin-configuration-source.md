# 插件配置的源代码派生

本增量实现纯值 `pluginConfig(kind, input).withParameters(values)` 与效果器 `.withHost({mix?,bypass?})`。
`kind` 为 instrument/effect，input 接受现有 builtin helper 或 npm factory 的声明式 ref。
参数按原样字符串键合并，点号不是嵌套路径。构造/派生不加载库、不创建 DSP；数据冻结，
资源与 structured builtin state 保留；不可序列化状态、非有限数与不合法 host 值拒绝。
参数名称/物理范围仍由准确插件的原生 descriptor 在候选 compile 时权威验证。

Channel/Bus 仅在控制侧保留配置输入对象身份。求值器从受信任 Project 对象关联 capture，
局部 reference 必须被单次执行的 owner 边界包围；同一 owner 的重复共享使用不能猜选。
配置参数编辑不重排或替换实例。projection 采用 engine 1.1 的实例 handles，owner/slot
只描述当前显示位置，不写用户 TS。实例/typed automation 与重排见 [20](20-plugin-instances.md)；
typed runtime commands 与 ABI 2 仍待迁移。

Document `configuration` operation 带 site、可选 usage 和 parameters/host edit。默认局部使用，
定义范围显式共享；完整工程比较保证只改准确配置，其他实例、宿主同名参数、automation、
路由/资源/插件注册不变。失败保留原代码和 accepted graph。相同 wrapper 的重复参数编辑
合并 literal patch；保留函数调用、副作用、非 literal 参数、注释及 import/export。
writer 复用可见 pluginConfig 别名/namespace，避开 type-only 与遮蔽；普通 Save 仍走统一 journal。

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
