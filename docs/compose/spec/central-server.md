---
feature: central-server
status: delivered
updated: 2026-10-06
branch: master
commits: 工作区（未提交，基线 ee85650）
---

# 中心化服务器：服务端打标 + 版本标注（消 403）+ 下载中心内搜索

## Report

**What was built** — 中心索引升到 `schema 2`，每个默认游戏端（Mindustry 官方 / Mindustry v8 / MindustryX）由服务端下发 **名称 + logo + 标语 + 标签 + 版本快照**；`server/central.mjs` 承担全部打标与 GitHub 索引（分页拉全、滤除服务端产物、TTL 缓存、可选 `GITHUB_TOKEN`），产物 `server/index.json` 153KB 含 281 个版本，静态托管也能直接吃。客户端 `central.rs` 新增 `slogan` / `versions` / `versionsError` 字段与 `preloaded()`：**服务端标注过就直接用，一次网络都不打**；服务端没管的源（file-list 目录页）才由客户端解析。客户端同时补齐与服务端同规则的打标层（`derive_tags` / `kind_tag`）与标语兜底（`builtin_slogan` / `fallback_slogan`），保证同一源在线/离线看到同一套名称、logo、标语、标签；两边的内置 id 也对齐（`src-mindustry-official` / `src-mdtbbs-v8` / `src-minedx`），否则版本缓存会互相对不上。下载中心新增**页内搜索**（源名 / 标语 / 标签 / 版本号，中心源与本地源表一起吃这个词，搜索时强制展开旧版本），并修掉搜索态下「命中旧版本却被标成最新」的假徽标。

**第二轮修订（同日，用户实机反馈）** — 下载中心只留「能下载到客户端」的东西：资产必须**严格以 `Mindustry` 开头**、**不是 apk**、且不是服务端产物（`dependencies.jar` / `assets.jar` / `desktop-release.jar` / `dexed-*.loader.jar` / `*-Android.apk` 全部剔除；官方 203→**143** 版、MindustryX 78→**38** 版），**过滤后一条客户端资产都不剩的版本整条隐藏**（不再显示空版本行）；「已滤除 N 个服务端」徽标按用户要求删除。修掉「v8 目录页第 1 条 `build-147-prerelease` 被当成最新版」——目录页是旧→新，`parse_file_list()` 现在按 `ver_key`（首段 major.minor）降序排，`build-160.5-stable` 正确置顶。新增 `RULES_VERSION` 缓存规则版本：规则一变老缓存整份作废，否则用户会在 15 分钟 TTL 内继续看到旧过滤结果（实测：带 203/78/43 旧数据的无 `rules` 字段缓存被正确丢弃并重扫成 143/38/43）。中心索引改为**开机即拉**（`store.loadCentralIndex()` + `App.vue` onMounted），进下载页不再等一次网络。下载页顶部大标题「下载中心」按用户要求删除；不可达提示改为「中心化服务器不可达：curl 退出码 7；已用内置索引」。

**Verification** — `cargo test` **26/26 PASS**（新增 4 例：客户端打标与服务端同规则、服务端标语优先且客户端补齐、服务端快照跳过网络、远程索引含 slogan/versions 的端到端解析）；`npm run build`（vue-tsc + vite）PASS、`npm run check:dist` PASS（自包含 517306 B）。体积：**YYL.exe 5.08MB**（Electron 壳 180.04MB，仍是最小的启动器 exe）/ NSIS setup **1.99MB**。实机 CDP（`YYL.exe` + `--remote-debugging-port=9333` + 本机 8787 中心服务）：三条源的**标语与 logo 全部上屏**（Anuken 官方原版 · 紧跟上游 stable / mdtbbs 文件站 · 国内直连目录 / TinyLake 分支 · 桌面端增强整合），标签依次为 `仓库,中心,官方` / `目录,中心,v8` / `仓库,中心,GitHub`；客户端同步 `ok 3/3`、`syncedAt 03:46:27`、耗时 8005ms、**`errors: {}`**——官方 203 条与 MindustryX 78 条直接来自服务端快照，file-list 43 条由客户端解析，全程没有任何客户端到 `api.github.com` 的直连；搜索 `mindustryx` → 命中 1 个中心源且只渲染该源；搜索 `v160.3` → 命中 1 个中心源、置顶卡片为 `row-card`（不冒充最新）+「搜索命中」灰标；搜索 `zzz-nothing` → 「没有匹配…」空态；点「清空」→ 复位为 3 个源、计数条隐藏。**该机唯一未上屏的是 mdtbbs 的 logo**：系统代理（见 Journey log）对 `file.mdtbbs.cn` 不可达，`<img>` 触发 `@error` → 回退首字母「M」，正是设计好的兜底路径；同机探测 `github.com/Anuken.png` 为 `ok 96x96`。

**第二轮 Verification** — `cargo test` **31/31 PASS**（新增 5 例：只留客户端资产的判定、空版本整条隐藏、目录页降序、`ver_key` 取 major.minor、老规则缓存作废）。实机三页读数：`pageTitle=(已删除)`、`toolbar=false`、全页 `已滤除` 文本 **0 处**；官方最新版 `v160.5`、按钮只有 `Mindustry.jar · 84.8 MB`；v8 最新版 **`build-160.5-stable`**、按钮为「打开目录」；MindustryX 最新版 `prerelease-2026.10.02.B502`、按钮只剩 `Desktop-SDL3.jar` / `2026.10.02.B502-Desktop.jar`。缓存规则守卫实测：先留一份**无 `rules` 字段、含旧数据 203/78/43** 的缓存 → 启动后变成 `rules=2`、`143/38/43`、`errors {}`。开机索引实测（服务端加访问日志）：**停在首页 10s 就打出 1 条 `GET /index.json`**，随后点「下载」并停留 3.5s，服务端**再没有新请求**，下载页 3 张源卡片立刻在位、`正在拉取中心索引` 从未出现。搜索回归：`v160.3`→`搜索命中` 且不加粉边、`mindustryx`→1 源、`zzz`→0 源、清空→3 源。

**Journey log**
- **403 的根因是系统代理，不是代码**：本机 `curl https://api.github.com/...` 直连 **200 / 0.68s**，加 `--proxy 127.0.0.1:7897` 立刻 **403 / 0.37s**；注册表 `ProxyEnable=1 / ProxyServer=127.0.0.1:7897` 说明 WebView2 与「带代理的 curl」都走这个出口。用户设置里的代理一开，客户端自己打 GitHub 就必踩 403——所以「客户端自己索引」这条路无论怎么写都会 403，必须把索引搬到服务端。
- 同一个系统代理还解释了另一处现象：WebView2 里对 `file.mdtbbs.cn` 的 `<img>` 探测（svg / ico / 站点根）**全部 error**，而同一时刻 shell 的 `curl` 拿得到 `200 image/svg+xml 353B`。即 WebView 走系统代理、curl 走直连，两者可达性不同；该源 logo 因此落到首字母兜底。这是环境问题，不是索引或前端缺陷（前端另有断言：`github.com` 的 png 在同一 WebView 里 `ok 96x96`）。
- 未带 token 的 GitHub 限额只有 60 次/小时，而客户端原本每个仓库要翻 5 页 → 一开机就可能打满。服务端侧加 15 分钟 TTL 缓存（`YYL_GH_TTL_MS`）+ 可选 `GITHUB_TOKEN`，客户端一次请求都不发。
- `versionsError` 的语义是「服务端试过了、失败了」，客户端据此**不再自己重试网络**（宁可显示服务端给的原因并保留本地缓存，也不去踩一遍 403）；file-list 不下发且不报错，客户端才知道这个源该自己解析——三者靠 `versions` 空 + `versionsError` 空来区分。
- 服务端 `slug()` 从中文名生成 id（`mindustry-官方`），与客户端内置 id（`src-mindustry-official`）不一致，会让在线/离线两套版本缓存互相孤儿化；改为服务端显式写 id 对齐。
- **「v8 最新版是 147」不是排序偏好问题，是页面顺序问题**：mdtbbs 目录页 43 条的**第 1 条**就是 `build-147-prerelease`、**最后一条**才是 `build-160.5-stable`（旧→新）。原来直接取 `[0]`，等于把最老的当最新。
- `ver_key()` 第一版有 bug：主版本循环用 `chars.next()` 退出，把小数点一并吃掉，`build-160.5-stable` 解析成 `(160, 0)`。测试 `version_key_reads_major_and_minor` 直接抓到（`left (160,0) != right (160,5)`），改成全程 `peek()` 后通过——这类「break 时已经消费掉下一个字符」的错，靠肉眼很难发现。
- 过滤规则升级后，用户的旧缓存会在 15 分钟 TTL 内继续显示老结果（服务端包还在、147 还挂着「最新」），看起来像「改了没生效」。加 `RULES_VERSION`：缓存里的规则版本对不上就整份作废重扫，同时也覆盖了旧版本写出的**没有该字段**的缓存（serde 默认 0）。
- 打包被「拒绝访问 (os error 5)」挡住：`cargo` 已经把新二进制产出为 `yueyue-launcher.exe`，但 tauri 要把它改名成 `YYL.exe`，而用户当时正开着旧的 `YYL.exe`（pid 3536）。Windows 下运行中的 exe 是独占锁——**编译前必须先关掉应用**。
- 搜索态下 `latestOf()` 取的是「第一条命中」而非「真·最新」，导致搜 `v160.3` 时它顶着「最新」徽标上屏；加 `isTrueLatest()` 判断（比对全量列表首条），命中旧版本时改为「搜索命中」灰标且不加粉边。
- 「进下载页会不会重新索引」不能用「点完立刻读 DOM」来验证：Vue 的视图切换在微任务里 flush，`click()` 之后同一个同步表达式读到的还是旧 DOM（当时读到 `cards: 0`，误以为没生效）。改成在服务端打访问日志计数，才拿到「开机 1 次、进页面 0 次」的确定结论。

## [S1] Problem

中心化服务器这一版要一次性解决四件事，且四件事互相耦合：

1. **403**。用户设置里配了本地代理，出口 IP 被 GitHub 拦；客户端 `sync_central_versions` 原本直接请求 `api.github.com`（每仓库最多 5 页），一开机就是 403 + 限流。用户明确要求「**只做服务端打标签**」，即把标注（含 GitHub 索引）搬到中心服务器，客户端只读结果。
2. **每个默认游戏端要有 logo + 标语 + 名称**。上一轮只补了 logo（且未编译未验证），标语字段根本不存在；三个默认源的 id 在服务端与客户端还是两套。
3. **标注分层**。用户决策为「**客户端和服务端都打标签**」并「两个都需要」：服务端是权威来源，客户端在服务器不可用退回内置索引时用**同一套规则**补标，标语同理（服务端优先 + 客户端兜底）。
4. **搜索**。用户选定「**下载中心内搜**」——不加全局搜索、不加顶栏搜索，就在下载页里搜源名 / 标语 / 标签 / 版本号。

## [S2] Design

### 1. 索引 schema v2（服务端权威）

```json
{ "schema": 2, "app": "yueyue-launcher", "generated": "…", "count": 3, "items": [{
  "id": "src-mindustry-official", "name": "Mindustry 官方", "kind": "github-repo",
  "url": "https://github.com/Anuken/Mindustry/releases", "repo": "Anuken/Mindustry",
  "asset": "Mindustry.jar", "note": "…", "group": "中心",
  "tags": ["仓库","中心","官方"], "size": 0,
  "logo": "https://github.com/Anuken.png?size=96",
  "slogan": "Anuken 官方原版 · 紧跟上游 stable",
  "scope": "client",
  "versions": [ { "tag","title","date","pageUrl","assets":[{"name","url","size"}],"dropped","folder" } ],
  "versionsError": ""
}] }
```

- `id` 显式写死并与客户端 `builtin_items` 一致，避免在线/离线两套缓存键
- `slogan` 缺省为空串；客户端按 `#[serde(default)]` 宽松解析，旧客户端读 v2 不会崩
- `versions` 是**服务端标注好的快照**；`versionsError` 非空表示服务端试过且失败
- 三者组合决定客户端行为：`versions` 非空 → 直接用；否则 `versionsError` 非空 → 只报错不重试；两者都空 → 客户端自己解析（file-list）

### 2. 服务端打标与版本标注（`server/central.mjs`）

- `tagsOf()`：`类型标签 + 分组 + 源自带标签` → 去空 → 截 16 字 → 去重 → 封顶 8
- `fetchGhVersions(repo)`：`/releases?per_page=100&page=1..5` 分页拉全，`isServerAsset()` 滤掉 server/dedicated 产物并计数 `dropped`；失败抛错由 `annotateVersions()` 降级为 `versionsError`
- 缓存：`server/.cache/<owner>_<repo>.json`，TTL 默认 15 分钟（`YYL_GH_TTL_MS=0` 可强制重拉），已加入 `.gitignore`
- `GITHUB_TOKEN` 提高限额；`NODE_USE_ENV_PROXY=1`（Node 24+）可让 `fetch` 走 `HTTPS_PROXY`
- `direct-url` 源本地合成单条版本；`file-list` 不下发（HTML 目录页无 API 限额，客户端解析）
- `build` 写 `server/index.json`（静态托管快照）；`serve` 每次请求重生成，版本走缓存

### 3. 客户端打标与快照优先（`src-tauri/src/central.rs`）

- `CentralItem` 增 `slogan` / `versions` / `versions_error`
- `derive_tags()` / `kind_tag()`：与 `tagsOf` 同序同规则；`sanitize()` 用服务器 tags/slogan，缺了才补
- `builtin_slogan(id)` + `fallback_slogan(it)`：内置表命中用内置文案，陌生源按类型给通用文案（封顶 60 字）
- `preloaded(it)`：`versions` 非空 → `Ok`；`versionsError` 非空 → `Err`；都空 → `None`（交给 `versions_of`）
- `sync_blocking()` 在并行线程里先问 `preloaded()`，服务端标注过的源零网络

### 4. 下载中心内搜索（`DownloadsView.vue` + `style.css`）

- 一行搜索框：源名 / url / 备注 / **标语** / 标签 / 版本 tag / 版本标题 / 资产名，全部大小写不敏感
- 源本身命中 → 该源全部版本；只有版本命中 → 只留命中的版本；搜索时强制展开旧版本、隐藏「展开旧版本」按钮
- 本地源表吃同一个词（名称 / 仓库 / 直链 / 备注），整组没命中不显示组头
- 「没有匹配」空态：中心与本地两边都空才显示；点「清空」复位
- `isTrueLatest()`：置顶卡片只在**全量列表首条**时才加「最新」徽标与粉边，命中旧版本改为「搜索命中」灰标

### 5. 客户端可见性规则（第二轮）

- **什么算可下载资产**：`name.starts_with("Mindustry")` **且** 不是 `.apk`（忽略大小写）**且** 不是服务端产物（`server*` / `dedicated*` / `*-server*`）。服务端 `isClientAsset` 与客户端 `is_client_asset` 同一套规则。
- **哪些版本要显示**：`github-repo` 源过滤资产后**一条不剩的版本整条隐藏**；`file-list` 源条目是目录、本来就没有资产，原样保留；`direct-url` 源的资产名由用户自定，不做前缀假设。
- **目录页排序**：`parse_file_list()` 按 `ver_key()`（名称里第一段 `major[.minor]`）降序，保证首条是最新版；同版本号时按名称降序兜底。
- **缓存规则版本**：`RULES_VERSION: u32`（当前 2）。`usable_cache()` 发现缓存 `rules != RULES_VERSION`（含旧版本写出的、没有该字段的缓存，serde 默认 0）就整份作废重扫，避免 15 分钟 TTL 内继续显示旧过滤结果。
- **开机索引**：`store.loadCentralIndex()` 由 `App.vue` 的 `onMounted` 调用（与 `loadCentralVersions→syncCentralVersions` 并列），同一个 `centralServer` 只拉一次；`DownloadsView` 进来直接复用 store 里的 `centralItems`，不再触发网络。
- **UI 减法**：删掉下载页顶部大标题「下载中心」（导航项已表达同样信息）；删掉「已滤除 N 个服务端」徽标。

### 验证边界

- `cargo test`：打标规则一致性、标语优先级与兜底、`preloaded` 三态、远程索引含 slogan/versions 的端到端解析
- 实机：中心服务 8787 → 客户端同步无 403、标语/logo 上屏、搜索命中与空态、清空复位
- 构建链：`npm run build` + `check:dist` + `tauri build --bundles nsis`；exe < 20MB

## [S3] Out of Scope

- 服务端解析 file-list（HTML 目录页）——无 API 限额，仍由客户端解析
- 镜像命名（用户明确「这个先放哪儿不写吧」）、按名称选镜像重试
- 「给他们 qq 下 8 个，标注 v7 / v8 版本」的源扩充（用户明确先放一放）
- 服务端多用户鉴权、增量推送、WebSocket；`scope=server` 服务端产物接入
- 本地源表 UI 的标语字段（本地源没有标语来源）

## Tasks

- [x] T1: 服务端 schema v2 + `slogan` + 版本标注（分页/TTL 缓存/token/降级）+ id 对齐 — acceptance: `node server/central.mjs build` 产出含 slogan & versions 的 index.json (covers: S2 §1 §2)
- [x] T2: 客户端 `slogan`/`versions`/`versionsError` + 同规则打标 + 标语兜底 + `preloaded` 优先 — acceptance: `cargo test` 全绿（含 4 个新用例） (covers: S2 §3; depends: T1)
- [x] T3: 下载中心内搜索（中心 + 本地）+ 空态 + 清空 + `isTrueLatest` 修正 — acceptance: `npm run build` / `check:dist` PASS (covers: S2 §4; depends: T2)
- [x] T4: 编译 YYL.exe 并实机验证（标语上屏 / 同步 errors 为空 / 搜索三条路径） — acceptance: exe 可启动、CDP 读数符合预期 (covers: S2; depends: T3)
- [x] T5: 客户端可见性规则（严格 Mindustry 开头 + 非 apk + 非服务端；空版本隐藏；目录页降序；`RULES_VERSION` 缓存守卫） — acceptance: `cargo test` 31/31 通过；实机只显示客户端包、v8 置顶 160.5-stable (covers: S2 §5; depends: T4)
- [x] T6: 开机即拉中心索引 + 删「下载中心」大标题与「已滤除」徽标 — acceptance: 服务端访问日志显示开机 1 次、进下载页 0 次；页面无残留标题/徽标 (covers: S2 §5; depends: T5)

### 实测数字存档（2026-10-06）

| 指标 | 值 |
|---|---|
| YYL.exe | 5.08 MB（5331456 B） |
| NSIS setup | 1.99 MB（2083796 B） |
| Electron 壳对照 exe | 180.04 MB |
| cargo test | 第一轮 26/26 · 第二轮 **31/31** PASS |
| server/index.json | 153 KB，281 个版本快照（官方 203 / MindustryX 78） |
| 客户端同步 | ok 3/3、syncMs 8005、errors {} |
| 直连 GitHub API | 200 / 0.68s |
| 经代理 127.0.0.1:7897 | **403** / 0.37s ← 403 根因 |
| dist/index.html | 517306 B → 第二轮 516901 B 自包含 |

### 第二轮数字（2026-10-06）

| 指标 | 值 |
|---|---|
| 官方可下载版本 | 203 → **143**（只剩 `Mindustry.jar`） |
| MindustryX 可下载版本 | 78 → **38**（只剩 `*-Desktop.jar` / `*-Desktop-SDL3.jar`） |
| v8 目录页条目 | 43，页面顺序「旧→新」，置顶修正为 `build-160.5-stable` |
| 缓存规则守卫 | 旧缓存 `rules=undefined` + `203/78/43` → 重扫为 `rules=2` + `143/38/43`、`errors {}` |
| 开机索引 | 首页停留 10s 即 `GET /index.json` **1 次**；进下载页 3.5s **0 次** |
| 「已滤除」残留 | 全页 0 处；`page-title` / `toolbar` 均已移除 |

复验命令：`node server/central.mjs build`；
`npm run typecheck && cd src-tauri && cargo test`；
`node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`；
实机：`node server/central.mjs serve 8787` + `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` 启动 `YYL.exe`，用 `scripts/cdp-eval.mjs` 读数。
