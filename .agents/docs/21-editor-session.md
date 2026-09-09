# 编辑器未保存缓冲区客户端

在 Document 2.0 Unix bridge 上实现 `@oxitone/cli/editor`；与 GPUI 共用 Document Service，
不是另一份音乐 Model。客户端连接后接受完整当前 View，自动重连同一路径，新 session
丢弃旧 command handles/待确认非文本命令；本地未保存文本保留，绝不把旧语义请求重放。

每个编辑器 buffer 持有实际文本、本地版本和共同 baseline。远端不变时发送本地完整文本；
本地不变时提出带 expected editor version 的更新。由宿主正常 text edit API 应用，确认前
不假装编辑器已经采用。双边都改变同一 buffer 时保留 baseline/local/remote，明确冲突；
同字节回声只推进 baseline。请求中的完整文本与后续按键文本分开，迟来的 ack 不能清除
更新的输入。不同文件互不覆盖，提交串行并带最新 session/revision/requestId。
异步宿主已收到的 apply proposal 在其 editor version 被确认前保持不变；后续远端版本排在
确认之后，不能用新 proposal 替换正在应用的文本。成功的 correlated response 单独确认
该次提交文本作为 baseline，即使同一 socket batch 已含更晚的远端事件；拒绝不确认 baseline。

baseline 同时记录接受它的 sessionId。重启后远端若与 baseline 和 editor 文本均不同，
显示 sessionChanged 冲突并保留实际 editor 文本，即使旧服务已经接受过该文本。接受不等于
保存，不能把旧服务中的已同步未保存文本当作可覆盖的干净缓存。远端与 baseline 相同可
正常提交离线编辑；远端与实际文本相同则收敛。旧 session 的在途 apply 保留旧 session 身份。

选择保留本地/采用远端须针对客户端当前冲突 revision 和本地 buffer version；期间任何一端
再次变化使旧选择失效。Save/Undo/Redo 先等待本地文本提交，冲突、断线、未完成的编辑器
应用或非法草稿显式失败；磁盘持久化仍只由服务器 journal 执行。客户端不写项目磁盘。
每文件 8 MiB、总 buffer 32 MiB、4096 files、网络 frame 64 MiB、有界重连与等待。

## VS Code adapter

`packages/editor-vscode` 是私有 VSIX 工程 `oxitone-vscode`，esbuild 将控制客户端打包成 CJS，
只 externalize `vscode`，不加载 native engine。扩展要求 trusted local workspace。启动命令
通过 ProcessExecution（不用 shell 字符串）运行项目 CLI，传 `--document-socket`；专用临时
目录 0700、socket 0600。也可连接 `oxitone daw` 输出的已有 socket。同机多个 session 共存。

linked `oxitone:` URI 的 authority 是 socket 的派生 key，不写 TS。路径保留原文件路径。
SourceFileSystem 仅提供现有登记文件，writeFile 调用 Document Save；Cmd+S、Save All、auto
save 不能绕过 journal。普通 `file:` tab 仍只有 disk watch，启动前要求保存原始 dirty tabs。
不能用 onWillSaveTextDocument 冒充可取消普通文件保存。Create TypeScript Source File 命令
通过 Document `createFile` 创建 `.ts`/`.mts` 内存文件并打开 linked buffer；`DocumentView.projectRoot`
是必填字段，由服务给出，客户端不从文件排序猜根目录；缺字段的事件在协议校验时拒绝。
Undo/Redo 可移除/恢复新文件，干净且在同一 session 被移除的打开 buffer 不阻塞项目历史；
继续在已移除 buffer 输入会阻止 Save，跨 session 缺失文件也不自动丢弃。rename/delete/Save As 未开放。
文件在 Save 前就能被其他 linked 模块 import/re-export；磁盘创建仍只由 journal 执行。

新建文件、插件任务与 Save/Undo/Redo 共用客户端命令互斥；先提交已有文本，命令在途时保留
后续键入，响应后用最新 revision 继续提交，第二个语义命令明确拒绝。普通请求等待 15 秒，
插件任务等待 150 秒以覆盖服务器的 120 秒执行上限及重建；断线仍不重放命令。

onDidChangeTextDocument 提交实际文本和 version；远端用 WorkspaceEdit 应用最小连续文本
差异，保持公共前后文且不切开 UTF-16 surrogate/CRLF。构建并提交 edit 同一同步调用，VS Code
内置 bulk edit 的文档版本前置条件在主线程应用时校验，竞争编辑拒绝。每项 edit 带普通 UI
label metadata，跳过 VS Code 1.96 的异步 minimal-edit pass（该 pass 丢弃 versionId，真实
并发测试已复现）；扩展已自行计算最小差异。打开时使用 readFile 实际提供的 baseline，
避免 read/open 之间的 DAW 更新被旧加载文本回退。成功 change event
推进共同 baseline，未成功不承认远端已应用。Open buffer 不依赖 filesystem reload 事件。
Save 串行、先 flush，保存前后核对 buffer 文本/version；期间新按键不能被标记为已保存。

项目树、同步状态、accepted/source revision 与项目 diagnostic 显示真实服务状态。冲突命令
展示只读 remote/base 与实际 editor diff，再提供保留 editor/采用 DAW；选择带原版本校验。
项目 Undo/Redo 命令与快捷键走 Document history；VS Code 普通文本 Undo 是新的 code 事务，
不是回放服务历史。断线自动重连，旧语义命令不重放，local baseline 和 socket 保存在本地
workspace state。无效草稿保留，当前 journal Save 仍要求 ready；没有假装实现“保存无效草稿”。
workspace state 还保存每个 baseline 的服务身份；恢复 dirty buffer 时使用它，旧缓存缺身份
时保守要求冲突检查。关闭 dirty/未同步文档仍保留 baseline，支持宿主语言切换时的 close/open。
这不是 server-only 未保存修改的 crash journal，也不代表完整 hot-exit/window reload 已验收。

真实 Extension Host 测试覆盖 buffer 版本竞争、未保存代码求值、DAW 音符编辑到文本、
FileSystemProvider Save/auto save、共享 Undo/Redo、后台 tab、同 socket 新服务重连与禁止
依赖写入，以及旧服务接受但未 Save 的文本在重启后保留并阻止静默 Save；也执行真实
launch command → CLI → headless simulated GPUI；不打开音频。
跨 npm 的完整 TS language service、编辑器侧自动行级 merge、新目录/资产 Save 与大工程增量性能
仍是独立门禁。基于 schema 的语法着色不等于完整 IDE。
