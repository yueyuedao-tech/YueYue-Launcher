---
feature: ui-docks
status: delivered
updated: 2026-10-06
branch: master
commits: 工作区（未提交）
---

# 启动日志归档 + 下载弹窗层级 + Mod 隐藏 + 右下角任务圈

## Report

**What was built** — 四件事：

1. **启动日志归档到左下角圆圈**。启动日志从「浮层里的一次性数组」改成**按实例归档**的 `launchRecords`（`instanceId → { name, status, code, logs[], startedAt, endedAt }`），事件监听从浮层内部**上移到 App 层**——浮层收起后日志仍在继续写入。点「启动」时浮层照常弹出并实时滚日志，`launch_instance` 返回后**自动收起**（2s），左下角出现圆圈；圆圈带运行中数量角标、运行时有呼吸圈，点开是按游戏分条的归档列表：每条显示名称/状态/日志行数/时长，选中即可回看该实例的完整日志，并可直接「停止进程」「用浮层打开」「移除记录」「清空已结束」。多开时每个实例一份日志，互不覆盖。
2. **下载弹窗（「下载并创建」）升到最上层**：`z-index` 95，盖住下载任务抽屉（80）与普通编辑浮层（70）；右上角新增 **✕**，并支持 **ESC**（先关弹窗，再关任务抽屉）。下载 + 创建完成后**自动关闭**，任务归到右下角圆圈。
3. **Mod 页默认隐藏**，新增设置分区「开发者」，里面的「开发者模式」开关才放开导航里的 Mod（页面注明尚未完成，仅作预览）；关掉开关时若正停在 Mod 页会自动退回首页。
4. **下载任务入口从右上角图标改为右下角圆圈**（含活跃任务角标），点击仍从右侧滑出任务抽屉。

**Verification** — `cargo test` **38/38 PASS**；`npm run build` / `check:dist` PASS。实机（`YYL.exe` + CDP）：

| 项 | 实测 |
|---|---|
| Mod 默认隐藏 | 导航 = 首页 / 游戏 / 下载 / 设置（无 Mod） |
| 开发者开关 | 打开后导航变为 首页 / 游戏 / 下载 / **Mod** / 设置 |
| 启动浮层 | 点启动后出现「正在启动 sleeper-A」+ 4 行日志 |
| 自动收起 | 约 4s 后浮层消失，左下角圆圈出现，角标 `1` |
| **多开归档** | 再启动 sleeper-B 后角标 `2`；列表 = `sleeper-B 运行中 14 行 · 5s` / `sleeper-A 运行中 34 行 · 16s`，各看各的 |
| 切换条目 | 切到 A 显示它自己的 50 行（首行「请求启动游戏「sleeper-A」…」，末行 `[err] [sleeper-err] warn 24`） |
| 停止进程 | A → `已停止 51 行 · 24s`，末行「[启动器] 已停止进程」，角标回 `1`（B 仍在跑） |
| 弹窗层级 | 探针元素实测：下载弹窗 `95` > 任务抽屉 `80` > 普通编辑浮层 `70` |
| ESC | 第一次关弹窗（抽屉保持），第二次关抽屉 |
| 右上角 ✕ | 关闭弹窗 |
| **自动关闭** | `1.9–3.9s 下载中 … 8 线程` → `4.9s 正在创建游戏…` → `5.9s 已创建游戏` → `6.9s 弹窗已自动关闭` |
| 弹窗进度显示 | 修复后：`0% → 18% → 25% → … → 100%`，速度 `7.1 → 5.0 MB/s`，8 线程 |
| 新增源立刻生效 | 缓存新鲜（28s，含 3 个源）时给服务端加第 4 个源 → 重载后缓存变为 4 个源（`tmp-autoclose:1`），无需等满 15 分钟 TTL |
| **下载中点 ✕** | 下载到 `22% · 3.4 MB/s · 8 线程` 时点 ✕ → 面板关闭、圆圈角标 `1`、下载继续 |
| **关面板后仍建游戏** | 面板被 ✕ 关掉后下载跑完 → 实例 `Popup Test big.jar` 照常创建，产物 20971520 B、无 `.part*` 残留 |
| **点「查看下载任务」** | 下载中点击 → 面板收起、抽屉打开且该点最上层元素就是 `.task-drawer`（`elementFromPoint` 验证，非“打开但被盖住”）、列表显示 `Popup Test big.jar` |

**Journey log**
- **「下载中点 ✕ 关不掉」**：`closePanel()` 里写着 `if (state === 'downloading' || 'creating') return` —— 当初是怕下载中关掉面板会丢进度。但现在已经有了右下角任务圆圈兜底，这个守卫就只剩副作用了。改成：关面板只是把 `dlPanelOpen` 置 false，**下载/创建照跑**，任务在圆圈里继续；同时把「下载并创建」的意图从面板对象里拆出来，按文件名存进 `pendingCreates`，这样**关掉面板也不会丢掉“等它下完就建游戏”**（实测：22% 时点 ✕ → 面板关闭、圆圈角标 1 → 下载跑完后实例照常出现）。
- **「点查看下载任务没反应」是我上一轮引入的回归**：按钮只做 `taskPanelOpen = true`，而上一轮为了「弹窗显示在下载页面之上」把弹窗提到了 `z-index 95`（抽屉是 80）——于是抽屉确实打开了，却开在弹窗**后面**，肉眼等于没反应。改成 `viewTasks()`：先收起面板再开抽屉；并用 `document.elementFromPoint(抽屉中心)` 验证最上层元素就是抽屉本身（不是「打开了但被盖住」）。
- 上一轮那个「同特异性看声明顺序」的 z-index 教训在这里又咬了一口：**提升某一层的层级时，必须同时检查所有「从别处打开这一层」的入口**，否则会把别的交互压死。
- **`.dl-overlay` 的 z-index 根本没生效**：`.edit-overlay{z-index:70}` 与 `.dl-overlay{z-index:95}` 特异性相同，而前者在样式表里更靠后 → 后者被覆盖，实测计算值是 **70**（被抽屉压住）。改成 `.edit-overlay.dl-overlay`（特异性 0-2-0）后才拿到 95。这类坑只改属性值、不看声明顺序是查不出来的；用一个临时插入 body 的探针元素读 `getComputedStyle` 可以在不依赖界面状态的情况下确认级联结果。
- **下载弹窗里一直显示 0%**：`confirmDownload` 只 `invoke('start_download')`，没有像 `startUrl` 那样先写一条 `downloadStates[fileName]`；而 App 层的进度监听是 `if (s) {...}` 才更新，于是百分比/速度永远是 0 与「测速中…」（线程数因为回退到面板自己的值所以看起来正常）。实机采样时才发现——**下载明明成功了，面板却一直 0%**。
- **15 分钟版本缓存 TTL 会把「新增的源」挡住**：给中心服务器加一个新源后，客户端因为缓存还新鲜直接复用，新源的 id 不在缓存里 → 界面上有源卡片却没有版本、也就没有下载按钮。改成：缓存新鲜时仍用 **5s 短超时**探一次索引，发现不认识的 id 就照常全量同步；探不通（离线/服务未起）直接沿用缓存，不为一次探测把启动拖成 15s。
- 本地源表的「下载」按钮走的是**行内状态机**（下载/取消/重试），**不弹**「下载并创建」面板；只有中心索引的版本按钮才走 `askDownload`。所以验证弹窗必须用中心源，用在 sources.json 里塞一条 direct-url 的土办法是测不到的。
- 手工写 `sources.json` 又会踩 BOM/编码两个坑：`Set-Content -Encoding utf8` 带 BOM 会被 serde 拒（已加 `strip_bom`），而中文经命令行传进 PowerShell 再写成文件会变成乱码——验证用的测试源最后改成纯 ASCII 名字才稳定。
- CDP 表达式自身的坑：`{3.8秒后: ...}` 这种以数字开头的对象键是非法 JS；PowerShell 双引号里 `\"` 不是转义（会提前结束字符串），要用 `[aria-label]` 之类的无引号选择器绕开。
- 验证期间用户一直在操作同一个窗口（切页面、点开关），会出现「读数时页面已经不在我以为的那一页」「弹窗被切页卸载」这类竞态。凡是依赖界面状态的断言，都改成**在同一个表达式里完成导航+操作+采样**，或者改成不依赖界面状态的探针。

## [S1] Problem

用户要求：启动游戏的日志浮层在**启动完成后收进左下角一个圆圈归档**，并且**每个游戏一份列表**——玩家多开时能分别回看；下载那边的「下载并创建」新窗口要**显示在下载页面之上**并允许 **ESC / 右上角 ✕** 关闭；**Mod 页先隐藏**，写一个开关放进设置的**开发者项**里预览这个没写完的版本；点下载并把游戏下载完成后**自动关闭**并归档到**右下角小圆圈**，把下载图标移到那边，点击查看所有下载任务。

现状约束：启动日志只存在于浮层的局部状态里（浮层一卸载就没了），且 `store.launch` 是**单实例**结构——多开时后一个实例会覆盖前一个的日志；下载弹窗用 `.edit-overlay`（z-index 70）低于任务抽屉（80），也没有 ESC/✕；导航里 Mod 是写死的五项之一。

## [S2] Design

### 1. 启动记录按实例归档

- `LaunchRecord { instanceId, name, status: running|done|failed|stopped, code, logs[], startedAt, endedAt }`，`launchRecords: Record<id, LaunchRecord>`
- `store.launch` 退化为「浮层开不开 + 看哪一条」（`open` / `instanceId`），不再持有日志
- 事件监听上移到 `App.vue`：`launch-log → appendLaunchLog(id, …)`、`launch-exit → setLaunchExit(id, code)`
- `resetLaunch(id, name?)` 新建记录；`openLaunchLog(id)` 已有归档则复用（重复打开**不清空**历史日志）
- `markLaunchStopped(id)`：后端对被 stop 的实例**不发** `launch-exit`，状态必须在前端落定
- `markLaunchFailed(id, msg)`：拉起进程本身失败（jar 缺失等）
- 单条日志上限 5000 行，避免长时间挂机吃满内存
- `autoCollapseLaunch(id, 2000)`：启动成功后自动收起浮层
- `LaunchDock.vue`：左下角圆圈（角标=运行中数量、运行时呼吸圈）+ 归档面板（列表 / 日志 / 停止 / 移除 / 清空已结束）

### 2. 下载弹窗

- `.edit-overlay.dl-overlay { z-index: 95 }`（必须双类，见 Journey log）
- 右上角 `.dl-close`；`window` 级 `keydown` 处理 ESC：先关弹窗、再关抽屉；`onBeforeUnmount` 解绑
- `createFromPanel` 成功后 1.6s 自动 `closePanel()`（下载中/创建中仍拒绝关闭）
- `confirmDownload` 先写 `downloadStates[fileName]`，弹窗才拿得到百分比与速度

### 3. 导航与开发者开关

- `ALL_NAV` 中 Mod 标 `devOnly`，`nav = computed(() => ALL_NAV.filter(n => !n.devOnly || settings.devMode))`
- 设置新增分区 `dev`（`SettingsTab` 加 `'dev'`），开关写 `settings.devMode`（默认 false，落 localStorage）
- `watch(devMode)`：关掉时若在 Mod 页则退回首页

### 4. 右下角任务圈

- 顶栏/侧栏的任务按钮删除，改为 `DownloadTasks.vue` 内的 `.dock-circle--right`（`position: fixed; right/bottom: 18px`），保留活跃任务角标，点击开合右侧抽屉

### 验证边界

- `cargo test` 全绿；前端 typecheck/build/check:dist
- 实机：启动→收起→圆圈、多开两条独立日志、停止落状态、弹窗层级/ESC/✕、下载创建后自动关闭、Mod 开关

## [S3] Out of Scope

- 启动日志持久化到磁盘（当前只在内存，重启应用即清空）
- 日志关键字过滤 / 导出 / 复制
- 多开的资源占用提示与并发上限
- Mod 页本身的功能完善（仍处于「未完成、仅预览」状态）

## Tasks

- [x] T1: 启动记录按实例归档 + 事件监听上移 + 自动收起 — acceptance: cargo/typecheck 通过；多开日志互不覆盖 (covers: S2 §1)
- [x] T2: `LaunchDock` 左下角圆圈 + 归档面板 + 停止/移除/清空 — acceptance: 实机两条独立日志、停止后状态正确 (covers: S2 §1; depends: T1)
- [x] T3: 下载弹窗层级 + ✕ + ESC + 创建后自动关闭 + 进度记录补齐 — acceptance: 级联实测 95>80；自动关闭实测 (covers: S2 §2)
- [x] T4: Mod 默认隐藏 + 设置「开发者」分区与开关 — acceptance: 开关前后导航差一项 (covers: S2 §3)
- [x] T5: 下载任务入口移到右下角圆圈 — acceptance: 右上角无图标、右下角圆圈可开抽屉 (covers: S2 §4)
- [x] T6: 版本缓存 TTL 挡住「新增源」的修复（短超时探查 + 新 id 触发全量同步） — acceptance: 新增源无需等满 TTL 即出下载按钮 (covers: S2; depends: T3)

### 实测数字存档（2026-10-06）

| 指标 | 值 |
|---|---|
| cargo test | **38/38 PASS** |
| 弹窗/抽屉/普通浮层 z-index | 95 / 80 / 70 |
| 自动关闭时间线 | 1.9s 下载中(8 线程) → 4.9s 正在创建 → 5.9s 已创建 → **6.9s 自动关闭** |
| 多开归档 | sleeper-A 50 行 / sleeper-B 34 行，独立查看与停止 |
| dist/index.html | 533955 B 自包含 |

复验命令：`cd src-tauri && cargo test`；`npm run build && npm run check:dist`；
`node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`；
实机：`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` 启动 `YYL.exe`，用 `scripts/cdp-eval.mjs` 读数。
