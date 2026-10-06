---
feature: settings-network
status: delivered
updated: 2026-10-06
branch: master
commits: 工作区（未提交）
---

# 设置重组：代理来源 / 中心化镜像 / 存档修复 / 个性化扩充

## Report

**What was built** — 按用户逐条要求改的一轮设置与交互整理：

**第二轮（同日）** — ①删掉「下载代理」旁的「重新读取」按钮（系统代理在开机与进入设置时各读一次）；②「新建游戏默认值」（JVM/游戏参数）从「个性化」移到「游戏」分区；③补全「关于」正文：版本、**月月岛科技**、以及六条架构说明（界面 Vue3+TS 打成单个自包含 HTML 内嵌进二进制 / 外壳 Tauri 2 + Rust（≈5MB vs Electron 180MB）/ 前后端 Tauri IPC / 外部依赖只有系统 curl 与用户自己的 java / 实例各自一目录并用 `AppData` 重定向实现存档隔离 / 中心化服务器负责打标与下发，客户端不直连 GitHub）；④下载页源卡片**删掉标语、URL、备注三行**，只留名称与标签（标签本就走中心化下发）；⑤**文件站支持真正的下载**。

**文件站下载（第⑤条，本轮最主要）** — 原来 mdtbbs 的目录页只能「打开目录」跳浏览器。勘察真实结构后发现三件事：目录页列的是**文件夹**（`class="term-file term-folder-link"`，名字在 `<div class="term-file-name">` 里），文件夹页列的是**文件**（`class="term-file"`，名字同样在 `term-file-name`，大小在 `<span class="size">`），而 href 指向的是**详情页**（返回 `text/html`），真正的 jar 在详情页的下载按钮上、指向 `/d/` + 同一路径。于是：`parse_term_entries` 统一解析文件夹与文件（名称优先取 `term-file-name`，兼容只有 `<span>` 的旧结构；大小解析 `84.8 MB` 这类写法），`parse_file_list` 只取文件夹（目录直接是文件时退化为「文件即可下载版本」），新增命令 `list_folder_files(url, proxy)` 把目录展开成可下载文件（换算成 `/d/…` 直链、滤掉服务端包）。前端在文件站版本行加「展开内容」按钮，展开后逐条列出文件名/大小/下载按钮，结果按 源+版本 缓存。

1. **下载代理变成来源选项**：新增 `proxyMode`（`system` = 读系统代理 / `custom` = 用自定义地址），自定义时显示输入框、系统代理时显示读到的值。新增 Rust `get_system_proxy`：先看 `HTTPS_PROXY/HTTP_PROXY/ALL_PROXY` 等环境变量，再读 Windows WinINET 注册表（`ProxyEnable` + `ProxyServer`，支持 `http=…;https=…` 形式）。所有网络调用统一走 store 的 `effectiveProxy`。
2. **工坊镜像改为中心化列表**：中心索引新增顶层 `mirrors.workshop`（`{name,url}[]`），服务端在 `server/central.mjs` 的 `MIRRORS` 里配置；客户端把清单随缓存一起留着（中心临时不可达也不会让设置页空掉），设置页改成下拉，未下发时显示「中心未下发镜像」。
3. **「中心化服务器」从「下载」分区移到「镜像与网络」**（它是源/网络配置，不是下载行为）。
4. **存档**：删掉「新开存档」功能（前端入口 + Rust `create_save` 命令一并移除）；修好「扫描存档点了没用」——真实存档在 `<实例>/data/Mindustry/saves`（启动器把 `AppData` 指向 `<实例>/data`，Mindustry 在其下再建 `Mindustry/`），老代码少了这一层，永远扫不到；非隔离实例改为读系统 `%AppData%/Mindustry/saves`。
5. **删除游戏共用本体提示**：新增 `sharing_jar` 命令，删除确认弹窗里列出还在用同一个 jar 的其它实例（含已隐藏），并写明「删本体留存档」不删磁盘 jar。
6. **个性化扩充**：界面缩放（90–150%，根元素 `zoom`）、背景模糊（0–30px，写 `--bg-blur` 并给背景加同比例放大避免边缘露底）、界面动画开关（根节点 `no-anim`，全局停用过渡/动画）、下载完成提醒（任务栏闪烁 + 允许时系统通知）、游戏退出时自动弹日志、新建游戏默认 JVM/游戏参数。并**修好窗口置顶与关闭行为的持久化**：原来只在模块加载时 watch(immediate)，可能早于窗口就绪而被静默吞掉；现在开机在 App 挂载后再落一次 `applyWindowSettings()`。
7. **右下角下载圆圈**：没有任务时**完全不显示**，有任务才出现并带角标，**去掉波纹效果**。
8. **设置新增「关于」分区**：logo + 名称 + 版本 + 内核，正文留成「待补充」占位，等用户给内容。

**Verification** — `cargo test` **41/41 PASS**（新增 3 例：`reg query` 的 REG_SZ/REG_DWORD 解析、名字前缀不误命中、存档路径必须落在 `Mindustry/saves`）；`npm run build` / `check:dist` PASS。实机（`YYL.exe` + CDP，测试数据：两个共用同一 jar 的实例 + 一个真实存档目录）：

| 项 | 实测 |
|---|---|
| 系统代理读取 | 切到「使用系统代理」→ 显示 **`系统代理 127.0.0.1:7897`**（注册表实为 `ProxyEnable=REG_DWORD 0x1` / `ProxyServer=REG_SZ 127.0.0.1:7897`） |
| 下载分区 | 只剩「下载目录 / 下载线程数」，中心化服务器已不在 |
| 镜像与网络 | 下载代理（来源下拉+读数）、中心化服务器、GitHub 加速前缀、工坊镜像（下拉 + 「中心未下发镜像」）、新版本小红点 |
| 设置分区 | 下载 / 镜像与网络 / 启动 / 游戏 / 个性化 / 开发者 / **关于** |
| **扫描存档** | `share-A` 下 `data/Mindustry/saves/MyTestSave` 被列出（修好前永远「暂无存档」） |
| 新开存档 | 已从界面移除 |
| **共用本体提示** | 删除 `share-A` 时提示「⚠ 还有 1 个游戏在用同一个本体 share-B」+ 说明不删 jar |
| 右下角圆圈 | 无任务时 `dock-circle--right` **不存在**；不再带 `live` 波纹类 |
| 界面缩放 | 设 125% → `documentElement.style.zoom = 1.25`，`getComputedStyle().zoom` 也是 1.25 |
| 背景模糊 | 设 12px → `--bg-blur: 12px` |
| 界面动画 | 关掉 → 根节点 `no-anim` = true；再打开 → false |
| **关闭按钮行为** | 设为「最小化到托盘」后 `CloseMainWindow()` → **进程仍存活（pid 不变）**、窗口隐藏，即真的最小化到托盘 |
| 关于页 | 渲染 logo / 名称 / 版本 0.1.0 / **月月岛科技** / 六条架构说明 |

**第二轮 Verification** — `cargo test` **45/45 PASS**（新增 4 例：文件夹/文件分流与名称不能取到「最近: …」、`/d/` 直链推导、大小解析、去标签）。实机：

| 项 | 实测 |
|---|---|
| 「重新读取」按钮 | 已不存在 |
| 新建游戏默认值 | 出现在「游戏」分区；「个性化」不再有 JVM/游戏参数 |
| 关于页 | 含「月月岛科技」+ 架构六条（界面/外壳/前后端通信/外部依赖/数据/中心化服务器） |
| 下载源卡片 | `Mindustry v8 目录中心v8` —— 标语、URL、备注三行全部消失 |
| **文件站展开** | v8 最新版出现「展开内容」；展开后列出 `[Android][v160.5]Mindustry.apk 75.0 MB` / `mindustry-linux-64-bit.zip 100.6 MB` / `mindustry-macos.zip 98.3 MB` / `mindustry-windows-64-bit.zip 97.3 MB` / `Mindustry.jar 84.8 MB`，共 **5 个下载按钮**，`server-release.jar` 已被滤掉 |
| **文件站真下载** | 点 `Mindustry.jar` 的下载 → 面板「下载并创建 · Mindustry v8 build-160.5-stable」→ 任务显示 **8 线程**、进度与速度（1.7–3.8 MB/s），总量 **84.8 MB**（与页面标注一致），磁盘上出现 8 个 `.partN` 分段文件；APK 那条也实测能下（75.0 MB，URL 含 `%5B…%5D` 转义同样可用） |
| 取消清理 | 取消后 `.part*` 残留 **0**，目标文件与未完成产物都不留 |

**第二轮 Journey log**
- **文件站的 href 不是文件，是详情页**：直接下 `/Mindustry/v8/…/Mindustry.jar` 拿到的是 `text/html`（6.4 KB），真直链在详情页的下载按钮上，是 `/d/` + 同一路径。若照 href 直接下，用户会下一个 6KB 的 HTML 当 jar。实测 `/d/…` 返回 `application/java-archive` + `Content-Length: 88902250` + **`Accept-Ranges: bytes`**，所以多线程分段下载在这里也能用。
- **详情页必须带 `--compressed` 才拿得到正文**：不带时 curl 报 200 却写出 0 字节（服务器发了 gzip，`Vary: Accept-Encoding`），带上就正常 6429 字节。已在 `http_get` 统一加 `--compressed`。
- **老解析器在文件页会把日期当文件名**：它取「最后一个 `<span>`」，而文件页最后一个是 `<span>最近: 2026-10-06 06:11</span>`。改成优先取 `<div class="term-file-name">` 的纯文本，并保留旧写法作为兜底（两个老测试用的正是简化结构，兜底让它们继续通过）。
- 「新建游戏默认值」挪分区时，前端只剩一个 `defaultJvmArgs` 引用点，`vue-tsc` 直接暴露了没清干净的地方——比 grep 可靠。

## [S1] Problem

**第一轮 Journey log**
- **`reg query` 的 `ProxyEnable` 是 `REG_DWORD` 不是 `REG_SZ`**：第一版解析器只认 `REG_SZ`，于是「系统代理已启用」永远读不到，界面显示「系统未设置代理（直连）」——而注册表明明是 `ProxyEnable=0x1 / ProxyServer=127.0.0.1:7897`。改成按类型标记（`REG_EXPAND_SZ`/`REG_SZ`/`REG_DWORD`）取余下部分，并加单测锁住两种类型与「名字前缀不误命中」。
- **「扫描存档点了没用」的根因是路径少一层**：启动器把 `AppData` 指到 `<实例>/data`，Mindustry 在 Windows 下用 `%AppData%/Mindustry/...`，所以存档真实位置是 `<实例>/data/Mindustry/saves`；老的 `saves_dir` 写的是 `<实例>/data/saves`。`mods.rs` 的 `mods_dir_for` 是对的（`data/Mindustry/mods`），只有存档这一处漏了 —— 同一个约定在两处写法不一致，是这次踩坑的根本原因。
- **窗口置顶/关闭行为「重启后不生效」**：`watch(..., { immediate: true })` 在模块加载时就调用 Tauri API，此时窗口可能还没就绪，promise 被 `.catch` 吞掉；Rust 侧 `CloseMode` 默认 `exit`，于是「最小化到托盘」永远失效。改成开机在 App 挂载后再显式落一次，并实测「关窗后进程仍存活」。

用户逐条要求：下载代理要能选「系统代理 / 自定义」；工坊镜像改为中心化列表；把「中心化服务器」从下载分区删掉（确认为移到镜像与网络）；个性化要更多功能（界面缩放、默认启动参数、下载完成通知、背景模糊与动画开关、退出自动弹日志）并修好置顶/关闭行为；删除游戏时要考虑多个实例共用一个本体；存档的新开功能删掉、扫描功能点了没用；右下角下载按钮无任务时不显示、有任务才出现且不要波纹；设置里加「关于应用」。第二轮追加：删「重新读取」、新建游戏默认值移到「游戏」、补关于正文（架构 + 月月岛科技）、下载卡片删标语/URL/备注、文件站要能真下载。

## [S2] Design

- **代理**：`Settings.proxyMode` + `Settings.proxy`；`systemProxy` ref 由 `get_system_proxy` 填充；`effectiveProxy` 计算属性作为唯一出口（下载/索引/工坊/更新检查统一用它）
- **镜像清单**：`CentralIndex.mirrors` 与 `VersionIndex.mirrors`（跟缓存一起留）；`sanitize_mirrors` 只收 http(s)、裁长度、封顶 20 条；服务端由 `MIRRORS.workshop` 配置
- **存档**：`saves_base(isolate, data_abs)` 统一口径（隔离 → `data/Mindustry/saves`；非隔离 → 系统 AppData）
- **共用本体**：`sharing_jar(id) -> Vec<name>`（扫实例目录 + 隐藏目录）；前端在删除弹窗顶部渲染告警
- **个性化**：`uiScale` / `bgBlur` / `animations` / `downloadNotify` / `autoOpenLogOnExit` / `defaultJvmArgs` / `defaultGameArgs`，各自有 apply 函数（zoom / `--bg-blur` / `no-anim` / 任务栏闪烁+通知 / launch-exit 时开浮层 / 新建表单预填）
- **关于**：新增 `about` 分区，内容占位

## [S3] Out of Scope

- 关于页正文（等用户提供）
- 工坊镜像的具体条目（等用户在 `MIRRORS.workshop` 里填）
- 系统代理的自动探测变更监听（只在打开设置/开机时读一次，带「重新读取」按钮）
- 非 Windows 平台的系统代理读取（仅环境变量分支）

## Tasks

- [x] T1: 代理来源选项 + `get_system_proxy` + `effectiveProxy` 全量替换 — acceptance: 切系统代理能读出注册表值 (covers: S2)
- [x] T2: 中心化镜像清单（服务端 + Rust + 设置页下拉） — acceptance: index.json 带 mirrors，界面空列表有提示 (covers: S2)
- [x] T3: 中心化服务器移到镜像与网络 — acceptance: 下载分区不再有该项 (covers: S2)
- [x] T4: 存档路径修复 + 移除新开存档 — acceptance: 造一个真实存档能被扫到 (covers: S2)
- [x] T5: 共用本体提示（`sharing_jar` + 删除弹窗） — acceptance: 两实例共用 jar 时出现告警 (covers: S2)
- [x] T6: 个性化扩充 + 置顶/关闭行为持久化修复 — acceptance: 关窗后进程存活；缩放/模糊/动画即时生效 (covers: S2)
- [x] T7: 右下角圆圈按需显示、去波纹；新增「关于」分区 — acceptance: 无任务时圆圈不存在 (covers: S2)

### 实测数字存档（2026-10-06）

| 指标 | 值 |
|---|---|
| cargo test | **41/41 PASS** |
| 系统代理 | `系统代理 127.0.0.1:7897` |
| 关闭行为=托盘 | `CloseMainWindow()` 后进程仍存活（pid 不变） |
| 界面缩放 125% | `zoom = 1.25` |
| 存档扫描 | 列出 `MyTestSave` |
| YYL.exe | **5.14 MB**；NSIS setup **2.01 MB** |
| dist/index.html | 541536 B 自包含 |
