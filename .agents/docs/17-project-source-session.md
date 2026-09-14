# 源码工程求值、监听与保存恢复

源码编辑只使用 `@oxitone/cli/source` 的 ProjectDocument；单 Pattern 文档、选定边界
evaluator 和其私有 worker 已删除。完整 GPUI 事务协议见 [18](18-project-daw.md)，
编辑器未保存同步见 [21](21-editor-session.md)。

## 打开与求值

```ts
import { ProjectDocument } from "@oxitone/cli/source";

const document = await ProjectDocument.open({
  entry: "/music/song.ts",
  projectRoot: "/music",
  readPaths: ["/music/assets/voice.wav"],
});
const site = document.view.sites.find((site) => site.label === "verse" && site.scope === "definition");
if (site) {
  await document.edit(document.view.revision, site.handle, [{ select: { degree: 2 }, set: { pitch: 65 } }]);
  await document.save(document.view.revision);
}
document.close();
```

entry 必须导出 Project、包含 Project 的结果，或返回它的 sync/async factory。
projectRoot 默认为 entry 目录；sourceRoots 显式登记 monorepo 源码包，projectRoot 也须登记。
打开先恢复 Save journal，再发现并读取源文件，禁止执行恢复前的半发布状态。

esbuild 对所有已登记模块使用内存 overlay；临时捕获 import 不写入作者源码。
捕获保留 this、await、调用次数与原始 import.meta.url。全新 Node worker 执行工程一次，
来源通过运行值映射；语义编辑要求选定边界唯一执行，不猜测重复调用中的某次输出。
无需 npm 源码或 sourcemap，也不会在用户源码加入 UUID、显式 ID 或绑定装饰器。

编译后的 worker 先加载宿主 JS，再启用 tsx；用户的 npm TS/TSX/CJS 仍通过同一 loader
执行。源码 worker 在启动时预载 tsx，后续 import 使用已加载的模块，不重复注册。
不复用执行过工程的进程或模块实例，不缓存 factory 返回值；超时和取消仍覆盖 loader 启动。

Node 22.15+ 的同步 module hooks 记录实际模块与祖先 package/lock；每次求值按目录扫描一次，
同时记录 package/lock 不存在的证据（临时 bundle 目录除外）。目录缓存不跨求值共享；
扫描后出现的新文件使候选失效。父进程记录
本地 TS/JS/JSON、已有解析配置和 readPaths。未保存模块另记录不存在的负读证据。
候选完成后复核文件路径/realPath/hash，并执行整个工程音乐等价与 native graph 校验。
读集 capture/check 分批最多并行 16 个文件，不用 mtime/size 替代内容散列；失败也等待当前批
全部结束，按原读集顺序报告首个错误，不再启动后续批次。各事务阶段的复核均保留。
总读集上限 4096，单源 8 MiB、工程 32 MiB、最多 4096 源文件。
任意 fs/网络/环境/时钟副作用、完整条件导出与其他新增解析配置（如 tsconfig）负证据仍未全部追踪。

用户执行上限 10 秒、返回上限 64 MiB、诊断尾部 16 KiB。超时或取消 SIGKILL，待退出
清理 bundle；执行限时不包含 esbuild 总耗时。进程隔离不是任意 JS 的安全沙箱。
Rust 校验使用无设备控制侧路径，accepted 不表示播放引擎已采用该图。

## 监听与外部冲突

DAW runner 使用 watchProjectDocument 监听源文件和已知读集的父目录，按文件过滤，
支持编辑器 temp+rename 保存。120 ms 防抖，求值/保存完成后再处理事件，关闭释放 watcher。

- 干净文件遇到外部保存时更新 baseline、递增 revision 并重新构建。
- dirty 文件遇到不同外部文本时保留草稿，记录 conflicts 并拒绝语义编辑/Save。
- resolveDiskConflict 核对所显示 diskHash；use-disk、keep-draft 或保守非重叠行级 merge
  建立新 baseline，清空过期历史；keep-draft 仍须 Save 才发布。
- 已知依赖变更以当前草稿重新求值；外部创建与尚未落盘的同路径草稿产生显式冲突。
- 本地 Save 回声按 baseline/read hash 去重；不会生成自身的外部代码事务。

代码 changeCode、图形编辑和 Undo/Redo 共用 ProjectDocument 代次检查。更晚代码可取消
旧求值；close 后的候选不能采用。普通 file: 编辑器 tab 只有磁盘监听，linked buffer
通过 editor bridge 同步未保存文本。目录重装/未知新依赖发现仍须后续完善。

## Save journal

Save 只持久化 accepted 且无冲突的工程；保存期间编辑拒绝 SourceChanged。
Save 不递增源码 revision，成功后更新 disk baseline/read set。无效草稿另存尚未支持。

SourceSaveStore 发布 1..64 个已登记 UTF-8 TS 文件，前后镜像预算 32 MiB，已有文件保留
mode，新文件 mode 为 0600。支持创建、修改和历史撤销删除；null 表示不存在，空串表示
已存在空文件。完整工程是正常 TS 与显式依赖，journal 不承载独有音乐语义。

只接受 journal version 2。其他版本拒绝并保留原文件和日志供检查，既不兼容转换，也不
自动删除未完成恢复数据。`.oxitone-source-save` 位于登记 root，不可为 symlink/依赖目录。

1. 获取协作写锁并恢复残留日志，固定 write/read set，核对 baseline、所有权与文档代次。
2. prepared journal 经 temp+fsync+rename+目录 fsync 发布，再复核读集。
3. 已有文件通过同目录独占临时文件和 rename 发布；创建以 no-clobber hard-link 发布，
   随后移除临时名；删除执行 unlink。各项均 fsync 并逐项核对 baseline。
4. 复查全部 postimage 和未写依赖，持久化 committed journal，再删除日志并 fsync。

prepared 恢复整套 preimage；committed 完成 postimage。第三种外部镜像不覆盖，保留日志
并返回 SourceChanged。journal 记录创建 staging identity，恢复校验 inode 后清理
link/unlink 间强杀留下的双链接。损坏镜像、路径重定向或未知链接一律拒绝。

锁保存 PID，活动 owner 排斥第二 writer；确定死亡后原锁内独占 recovery claim。
未知 owner/PID 复用/残留 recovery claim 保守拒绝，不以超时偷锁。
多文件发布可恢复但不跨文件原子；非协作编辑器仍可在核对和发布之间竞争，不承诺系统 CAS。

## 验证

project-session 测试覆盖真实 async factory、this/模块 URL、重复执行边界、过期求值/
语义候选、close 取消、超时和文件 watcher。source-creation/source-save 覆盖创建、Undo/Redo、
空文件冲突、SIGKILL、staging link、旧格式拒绝、第三方镜像、所有权和非 UTF-8 拒绝。
完整工程基准统一使用 `node packages/cli/bench/source-daw.mjs`（先 build CLI）；
不再维护单 Pattern 文档基准。测试与基准不打开音频设备。
