---
feature: download-page
status: delivered
updated: 2026-10-05
branch: master
commits: 5284505..fdf55fd
---

# 下载页（结构化源表 + GitHub 索引 + 镜像 + 个性化设置）

## Report

**What was built** — 下载中心完整落地：`sources.json` 结构化源表（kind=github-repo/direct-url、分组、折叠、latestEnabled/openInNewPage 开关，旧 subscriptions.json 只读迁移并复验，预置官方仓库+v146 直链双源）；GitHub `releases.atom` 版本索引（免 API 限额，手工解析零新依赖，行右侧「最新」徽标置首行）；curl.exe 真实下载（先 spawn 登记根除取消空窗、后台线程异步探测总大小、进度/完成/错误事件、取消双侧复位、幽灵失败标 -2）；全局 GitHub 加速前缀 + 空前缀「镜像重试」守卫提示；下载完一键创建实例。个性化设置 7+2 项全做：强调色四色主题（CSS 变量联动）、主页遮罩强度（--veil）、窗口置顶、关闭行为（退出/最小化到托盘，懒创建托盘+CloseRequested 拦截）、自定义下载目录、新版本小红点（启动静默 fetch+已读标记）、下载代理、加速前缀、更新开关。导航恢复四项（主页/实例/下载/设置）。新依赖仅 tray-icon 与 tauri-plugin-shell。

**Verification** — `cargo test` 9/9（Atom 样例解析、repo/asset/源名校验、迁移与预置往返）；typecheck/build/check:dist 全 PASS；体积 setup **1.93MB** / exe **4.89MB**（<20MB）。实机 CDP：迁移（无 BOM 夹具）→migrated-0、删除→预置双源、Atom 真拉 10 条首条 **v160.5**、红点亮起、accent紫→#c084fc/#818cf8、bgShade60→veil0.582、set_close_behavior minimize（托盘建成）；下载全链：v146 真下 69.6MB（20 条 progress→done，total 与 HEAD 一致）、UI 进度条→创建实例按钮、取消→复位→重下→完成全周期、downloadDir 覆盖落自定义目录、gh-proxy 前缀 5s 拉完 69.56MB；一键建实例 json 实读正确并保留为可用实例；错误态点「镜像重试」空前缀显示设置页指引。独立评审一轮 2 critical（取消卡死、镜像提示缺失）→ 修复+实测 → 复核 **PASS**（残余：取消落在登记前毫秒窗可能留孤儿 curl——人工时序几乎不可达，已披露）。

**Journey log**
- 取消卡死的根因有两层：HEAD(20s) 在登记前的空窗 + 晚生僵尸 curl 写文件——「先 spawn 登记 + 后台探测总大小」同时根除。
- 本机 curl `--retry` 耗尽后退出码为 0（stderr 仍报 7）：成功判定必须 `dest.exists()` 兜底，幽灵失败记 -2。
- 手写迁移夹具带 UTF-8 BOM 会被 serde 拒绝；真实旧文件是旧版 Rust 写出（无 BOM）不受影响，测试夹具改用无 BOM 写入。
- 公共镜像前缀对 releases.atom 多返回非 feed（ghfast/gh-proxy 实测），但文件下载正常（gh-proxy 69.56MB/5s）；错误文案已引导排查。
- 评审关闭状态的 T25 属流程内记录；复核结论以本 Report 为准。

## [S1] Problem

启动器缺少获取游戏客户端的途径，且用户需要：**本地稳定的源订阅表**（含分组/折叠/最新标记/是否提供页面按钮等元信息）、**通过 GitHub 索引仓库版本**（版本列表 + 最新徽标置右 + 逐版本下载按钮）、**镜像加速**（GitHub 前缀 + 失败重试）、以及一批**个性化设置**（强调色、置顶、关闭行为、下载目录、新版本小红点、主页遮罩强度等）。现实约束：GitHub REST API 未登录 60 次/小时且本机出口 IP 已被限流过，故版本索引走 `releases.atom`（无认证限额）；体积硬指标 ≤20MB 不变，新增依赖仅 tray/menu 与 shell-open 两个小组件。第一阶段（平铺订阅 + curl 下载骨架 + 一键建实例）代码已就绪，本修订将其升级为结构化源模型。离线约束澄清：启动器本体零联网依赖，所有网络行为均为用户显式触发或可开关的更新检查。

## [S2] Design

### 1. 结构化源表 sources.json

路径：`%APPDATA%\StarlightLauncher\sources.json`（Linux 走 XDG，与 instances 同规则）。

```json
{ "sources": [{
  "id": "s1", "name": "GitHub 官方仓库", "kind": "github-repo",
  "repo": "Anuken/Mindustry", "asset": "Mindustry.jar",
  "note": "官方 releases 索引", "group": "官方源",
  "collapsed": false, "latestEnabled": true, "openInNewPage": true
}, {
  "id": "s2", "name": "Mindustry v146 直链", "kind": "direct-url",
  "url": "https://github.com/Anuken/Mindustry/releases/download/v146/Mindustry.jar",
  "note": "官方 stable · 约 73MB", "group": "直链",
  "collapsed": false, "latestEnabled": false, "openInNewPage": false
}] }
```

- `kind`: `github-repo`（按仓库索引）| `direct-url`（现有直链卡片流程）
- `group` 分组展示，组头可折叠；`collapsed` 为该源初始折叠态，组头点击同步组内全部
- `latestEnabled`：github-repo 源是否在版本行右侧显示「最新」徽标（feed 首条=最新）
- `openInNewPage`：是否显示「页面」按钮（经 shell 插件调系统浏览器打开 release/资源页）
- 命令：`list_sources()`（无 sources.json 时：有旧 subscriptions.json 则逐条迁移为 direct-url 源，否则写入上述预置）、`save_sources(items)`（整表校验写回：名称非空≤60、direct-url 必须 http(s)://、repo 必须 `owner/name` 形态）
- 旧 `list_subscriptions/save_subscriptions` 命令删除（迁移只读旧文件，不回写）

### 2. GitHub 版本索引（Atom，免 API 限额）

- `fetch_repo_versions(repo, proxy, prefix) -> Vec<GithubVersion>`：curl 拉 `{prefix}https://github.com/{repo}/releases.atom`（prefix 即镜像前缀，见 §3；proxy 为设置值），**手工解析** XML（零新依赖）：逐 `<entry>` 取 title、`/releases/tag/<tag>` 链接、updated；最多返回 12 条（feed 顺序即新→旧）
- `GithubVersion = { tag, title, updated, pageUrl, jarUrl }`，`jarUrl = https://github.com/<repo>/releases/download/<tag>/<asset>`（asset 来自源字段，默认 `Mindustry.jar`；无该资产时下载会收到 404 错误事件，卡片显示失败可重试）
- UI：github-repo 源展开时（或点「刷新版本」）拉取；每行 = tag + 日期 + 行**右侧「最新」徽标**（仅 index0 且 latestEnabled）+「下载」按钮（文件名 `<repo尾>-<tag>.jar`，走现有下载状态机）+（openInNewPage 时）「页面」按钮

### 3. 镜像加速（全局前缀 + 镜像重试）

- 设置 `githubPrefix`（默认空=直连）：以 `https://github.com/` 开头的目标 URL 在**下载与 Atom 拉取**时自动前置拼接（如 `https://ghproxy.example/` + 原 URL）；输入框带 datalist 常用前缀示例
- 下载失败状态追加「镜像重试」按钮：仅当 githubPrefix 非空时可用，点击以前缀+原 URL 重新发起；前缀为空则提示先去设置页配置

### 4. 下载层调整（在既有 curl 方案上）

- `start_download(url, fileName, proxy, downloadDir)`：`downloadDir` 非空时覆盖默认下载目录（设置项 `downloadDir`）；运行表另存每个任务的目标路径供 stop 清理
- 进度/完成/错误事件、取消、状态机、一键建实例：维持第一阶段设计不变

### 5. 个性化设置（7+2 项全做）

| 设置 | 类型 | 生效方式 |
|---|---|---|
| 强调色 accent | 粉/紫/青/橙 | 监听写 CSS 变量 `--pink/--cyan`（`--grad` 引用二者） |
| 主页遮罩强度 bgShade | 0-100 滑条（默认 96） | 写 `--veil`，`.home-bg::after` 各档位按比例 calc |
| 窗口置顶 alwaysOnTop | 开关 | `getCurrentWindow().setAlwaysOnTop` |
| 关闭行为 closeBehavior | 退出 / 最小化到托盘 | Rust 侧 `set_close_behavior` 存状态；CloseRequested 时 minimize→prevent_close+hide，**按需懒创建托盘**（tray-icon+menu 特性，复用窗口图标，菜单=显示窗口/退出） |
| 自定义下载目录 downloadDir | 路径输入（空=默认） | 传入 start_download |
| 新版本小红点 updateCheck/lastSeenTag | 开关 + 已读标记 | 启动后台静默 fetch 最新 tag（失败静默）≠lastSeen 则导航「下载」亮红点；进入下载页即视为已读并写回 |
| 下载代理 proxy | 文本（已有） | 传入下载与 Atom 拉取 |
| GitHub 加速前缀 githubPrefix | 文本 + datalist 提示 | §3 |

- `tauri-plugin-shell`（npm + crate + capability `shell:allow-open`）：页面按钮系统浏览器打开
- `tauri` feature `tray-icon`（v2 菜单 API 内建、无需独立 feature）：托盘所需，体积影响计入验收
- 取消语义：`stop_download` 成功后由**前端复位该文件的下载状态**（后端 stop 不发事件，成功=回到可下载，「未在下载」错误=已完成则保留 done）

### 6. 前端结构调整

- 导航恢复四项（主页/实例/下载/设置），「下载」导航项承载小红点
- 下载页：按 `group` 分区（组头折叠/展开）、github-repo 可展开版本列表、direct-url 卡片、添加源表单（kind 二选一 + group + 三个开关）、删除源；下载状态机与进度条复用现有实现
- 设置页分组扩到：外观（accent/bgShade）、窗口（置顶/关闭行为）、下载（代理/目录）、更新（小红点开关）、GitHub（前缀）

### 验证边界

- `cargo test`：源校验、旧订阅迁移、Atom 解析（内嵌样例串）
- 实机：旧订阅迁移正确；Atom 拉取返回真实 v146+ 且首条拿「最新」；小文件经代理 progress→done；镜像前缀拼接 URL 可观察；官方 73MB jar 真下载 → 一键建实例；accent/bgShade/置顶/下载目录/红点逐项生效；托盘最小化→托盘恢复→退出
- 构建链 PASS + check:dist + 体积 <20MB；UI 冒烟（四导航、分组折叠、版本行）

## [S3] Out of Scope

- 断点续传、并发下载队列、sha 校验
- REST API 数据（体/作者等富信息）、仓库代码浏览（只索引 releases）
- 托盘图标自定义/多语言/主题色渐变编辑器
- Electron 壳接入本功能；Linux 实机验证（随 Linux 构建）

## Tasks

- [x] T1: Rust 源表层（结构/迁移/预置/校验）+ GitHub Atom 索引 + 下载层参数扩展（downloadDir/dest 注册） — acceptance: cargo test 通过（含 Atom 解析与迁移用例）、cargo check 通过 (covers: S2 §1 §2 §4)
- [x] T2: 托盘/关闭行为/置顶/shell-open 四件 Rust 侧（features+plugin+set_close_behavior+懒托盘+CloseRequested） — acceptance: cargo check 通过；实机最小化到托盘→恢复→退出链路可用 (covers: S2 §5; depends: T1)
- [x] T3: 前端——源表 UI（分组折叠/版本列表/最新徽标/页面按钮/镜像重试/增删源）+ 7 项设置接线 + 小红点 — acceptance: 构建通过；DOM 冒烟：四导航、组折叠、版本行含「最新」徽标 (covers: S2 §3 §5 §6; depends: T1 T2)
- [x] T4: 实机全链路验证 — acceptance: 迁移/Atom 真实版本/小文件代理下载 progress→done/官方 jar 真下载→一键建实例/accent与bgShade生效/红点亮起清零/托盘链路；测试产物处置明确 (covers: S2; depends: T3)
- [x] T5: 体积回归 + 独立评审 + 文档定稿 — acceptance: setup≤20MB；评审 PASS；status=delivered (covers: S2; depends: T4)

### 实测数字存档（2026-10-05）

| 指标 | 值 |
|---|---|
| NSIS 安装包 | 1.93 MB |
| exe（前端内嵌） | 4.89 MB |
| cargo test | 9/9 PASS |
| 官方 v146 jar 真下载 | 69.6 MB，进度事件 20 条 |
| gh-proxy 镜像前缀下载 | 69.56 MB / 5s |
| Atom 索引 | 10 条，首条 v160.5 |
| 取消→复位→重下→完成 | UI 实测全周期 PASS |

复验命令：`npm run typecheck && cargo test && node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`（系统 Node + 代理）；CDP 实机驱动脚本逻辑见 Journey log。
