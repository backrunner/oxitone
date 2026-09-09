# Pattern 源码所有权、import 与本地拆散

Pattern 源码 writer 是 [ProjectDocument](18-project-daw.md) 的事务组件；代码、GPUI 与
VS Code 统一使用完整工程文档。已删除未发布的独立单 Pattern 文档及其专用 evaluator，
不保留双份草稿、保存或历史状态。

## 文件所有权

`SourceOwnership.open(sourceRoots, files)` 登记现有源码；`assertWritable(path)` 每次重新
检查根目录和真实路径。只编辑 .ts/.mts，声明文件、CommonJS、依赖和生成目录拒绝。
内部符号链接可登记，但同一文件的多个别名、hard link、重定向链接和依赖链接不接受。
嵌套 package.json 划出独立包边界，monorepo 源码包须独立加入 sourceRoots。

新文件经 ProjectDocument.createFile 在已存在的父目录内创建内存草稿，检查预算、
revision/generation 和路径后才登记。真正创建和历史撤销删除由 Save journal 发布。
所有权失败为 EditNotRepresentable；这不是用户 JS 的安全沙箱或文件系统 CAS。

## 模块与拆散原语

`materializePatternReference(request)` 接受 PatternWriteRequest，先执行请求的音符编辑，
再产生普通 `new Pattern({ lengthBeats, name?, notes: [...] })` 候选。Note 不包含用户 ID、
source hash 或临时会话标记。候选报告音符数量、长度与失去生成器联动的影响。

绑定读取可以直接展开；调用/getter/await 通过逗号表达式保留原求值一次，再返回本地 literal。
不复制外部函数体，不改其他引用，不写 node_modules。保留原求值的展开仍执行原依赖；
作者随后移除无用 imports/生成器才会完全断开该依赖。

Pattern 构造器按作用域解析：复用可见的 oxitone/@oxitone/core named alias 或 namespace
运行时 import，不能复用 type-only 或被遮蔽的绑定。需要时从当前工程可解析的 SDK 包
新增不冲突的 value import，保持引号、CRLF、shebang 和 directive prologue。
普通 named/default exports 保持原结构，re-export 的被引用本地模块可在内存中编辑。
完整跨条件导出/tsconfig paths 的定义重构仍未实现。

`writeLiteralPatternEdit(request)` 直接更新可验证的 literal Note 数据，避免不断累加 edit。
非 literal、未知构造器或不能证明的表达式交由普通 writer 生成候选，不能静默改写不明函数。
两种 writer 均接受可选 lengthBeats，与音符一起写回；拆散保存新的长度。Document 根据
插入／改变时间的输出计算必要延长，向上取整到整数 beat。已有未变的长尾、力度／音高
修改和删除不触发延长。延长、音符修改、候选验证、Undo 和 Save 共用一次事务。

## 工程事务

writer 只生成候选文本；ProjectDocument 重新执行整个候选工程、验证 native graph，
比较音乐修改范围、无关配置和读集，全部通过才采用。源码与图形编辑共用一次 Undo 历史。

`planMaterialize(revision, site, edits?, placement?)` 返回包含 review 的文档投影；
确认使用当前 revision 和 planId，取消、源码修改、依赖变更、旧会话均使计划失效。
确认前不改草稿和磁盘。随机概率/时间窗口尚不能保证等价的场景明确拒绝。

现有 core source/writer、project-localization、project-session 和 source-review 测试覆盖
literal 回写、import alias/type-only/遮蔽、npm 拆散、共享引用保留、无依赖重开与过期候选。
文件保存和恢复见 [17](17-project-source-session.md)，完整交付状态见 [审查记录](../reports/2026-09-09-source-daw-review.md)。
