---
feature: download-tasks
status: delivered
updated: 2026-10-06
branch: master
commits: 工作区（未提交）
---

# 下载任务：多线程分段下载 + 速度/进度面板

## Report

**What was built** — 下载层从「单连接 curl + 按文件名记一个 Child」重写成**分段多线程下载器**：先用一个 `Range: bytes=0-0` 的请求同时问出总长与是否支持分段（`--max-filesize 1` 兜底，服务器若忽略 Range 回 200，curl 在读正文前就中止，不会为了探测把 85MB 拖下来），支持分段且体积 ≥4MB 时按 `split_ranges` 切成 N 段并发 curl 各自写 `.partN`，全部成功后按序合并、删除临时文件；任一段失败立刻收摊；服务器不支持 Range 或体积过小则自动退化为单连接。进度轮询累计各段字节数并按 250ms 计算**指数平滑速度**。新增后端**任务注册表**（`TaskInfo`：name/url/status/received/total/percent/speed/threads/path/code/startedAt）与 `list_downloads` / `clear_download` / `clear_finished_downloads` 三个命令；`start_download` 增加 `threads`（1/2/4/8，clamp 到 8）与 `name` 参数。前端在**顶栏右上角新增下载图标 + 活跃任务角标**（`navPosition=left` 时同一入口挂在左侧栏），点击从**右侧滑出任务抽屉**，每条任务显示名称/状态/进度条/百分比/已下与总量/**实时速度**/线程数，并可取消或移除；「下载并创建」弹窗新增**线程选择**、下载中显示速度与线程数、并带「查看下载任务」入口；设置页「下载」组新增**下载线程数**。所有下载动作（直链源、版本行、镜象重试、弹窗确认）都会带上线程数与显示名。

**Verification** — `cargo test` **38/38 PASS**（新增 6 例：切段覆盖完整且无缝隙无重叠、切段处理除不尽与超小体积、206+Content-Range 解析、Range 被忽略时回退、结束任务必须摘除登记、BOM 兼容）；`npm run build`（vue-tsc + vite）PASS、`check:dist` PASS。算法层另有独立验证脚本 `scripts/verify-segments.mjs`：本地起支持 Range 的静态服务，用**与 Rust 侧完全相同的 curl 参数**切 4 段下载 5MB+12345 字节（刻意留余数）再合并，**sha256 与源文件逐字节一致**。实机（`YYL.exe` + CDP + 本地限速文件服务）：

| 场景 | 结果 |
|---|---|
| 40MB / 支持 Range / 4 线程 | 落盘 41943040 B，**sha256 `3ec87c25…` 与源文件一致**，`*.part*` 残留 **0** |
| 20MB / 不支持 Range（自动回退） | 面板显示 **1 线程**，速度 **1.0–1.1 MB/s**（与 2s 内 2.6→4.8 MB 的字节增量吻合），**sha256 `c6f38437…` 一致**，残留 0 |
| 4 线程实时读数 | `8.1→14.3→20.3→26.4 MB` 每 2s ≈ 3MB/s，面板速度 **2.9–3.3 MB/s**，**4 线程** |
| 顶栏角标 | 点下载后立刻出现 `1`，任务结束后消失 |
| 分段探测 | `-r 0-0 --max-filesize 1` 对本地 Range 服务返回 `206 content-range: bytes 0-0/41943040` |
| 取消（下载中 12.7/40MB） | 抽屉任务**立刻消失且不复活**，`回环-Range.jar` 不存在、`*.part*` 残留 **0**、无残留 curl 进程 |
| 取消后重下 | 按钮从「取消」回到「下载」，点击后正常进入 4 线程下载 |
| 完成后重下 | 按钮为「重新下载」，点击后进入 4 线程下载（修复前必定报「已在下载中」） |
| 取消→重下→跑完 | 落盘 41943040 B，sha256 `3ec87c25…` 再次一致 |

**Journey log**
- **结束的任务没有摘除登记 → 同一个文件永远「已在下载中」**：重写时只顾着在完成/失败路径发事件、清临时文件，忘了把 job 从全局表里摘掉（旧实现是在 `active()` 里 remove 的）。后果是第一次下载成功之后，再点同一个文件的下载全部失败；而且前端 `startUrl` 的 catch 会把它标成 `失败 code=`（catch 只置 status 不置 code，所以 code 是空的），看起来像「下载坏了」。实机复现后加 `unregister()`（只在仍是自己那条时才摘，避免踩到新任务）+ 单元测试，并把前端 catch 改成「后端说已在下载中 → 以任务表对齐，不标失败」。
- 复现这条 bug 的过程本身也有坑：CDP 里「点下载」和「读面板」分两次调用时，中间的工具往返有十几秒，40MB 内网下载早就跑完了，看到的一直是「已完成」。后来把「点击 + 连续采样」放进**同一个表达式**才抓到真实过程。
- 分段下载的「幽灵成功」判定：`curl` 即便 `--fail`，在重试耗尽后也可能以 0 退出却没写全，所以合并前必须校验 `各段字节和 == 探测总长`，不等就报 -2 而不是当成功。
- 探测阶段不能用阻塞 `output()`：那会留下最长 25s 的**无法取消窗口**（旧代码专门为此把 HEAD 挪到后台线程）。这里把探测进程也登记进 job，`stop_download` 能立刻杀掉它。
- 多段并发的 stderr 交错没有可读性，各段统一 `stderr(null)`，错误一律靠退出码判断——顺带消掉了旧实现「stderr 设为 piped 却从不读取」的潜在阻塞。
- `ver_key()` 与 `split_ranges()` 这类纯函数是这次唯一能被单测直接覆盖的部分，边界（除不尽、段数>字节数、0 长度）全靠它们兜住。
- **用户手改的 `sources.json` 带 UTF-8 BOM 会被 serde 判成解析失败**，界面只显示「本地源表为空」，非常难查（`Set-Content -Encoding utf8`、部分编辑器默认写 BOM）。本轮实测踩到并加了 `strip_bom()`。
- 公网 GitHub 在本机不稳定：`github.com` 根路径 12s 超时，但 release 资源经 302 跳到 `release-assets.githubusercontent.com` 后 `Range` 可用（实测 206）。所以多线程的端到端验证改用**本地可控的 Range 服务**，不受公网抖动影响。
- **取消后 4 个 `.partN` 全部残留**：`stop_download` 先 `taskkill /F` 再 `remove_file`，而 Windows 上被杀的进程还没释放文件句柄，删除静默失败（目标文件因为没人持有句柄反而删掉了，所以现象是「半截分段文件留着、正式文件没了」）。改 `remove_with_retry()`（最多 12 次、60→150ms 退避）。
- **取消后抽屉里任务「复活」**：为了让角标立刻亮起来，`patchDownloadTask` 在找不到条目时会新建一条；取消之后后端仍有在途的 `progress` 事件到达，于是又把这条插了回来，看起来像取消失败。加 `canceledTasks` 集合：取消时打标、事件与刷新都跳过，重新下载时解标。
- 「点下载 + 连续采样」必须放进**同一个 CDP 表达式**，否则工具往返的十几秒里 40MB 内网下载早就跑完，只能看到「已完成」——第一轮就是这么误判的。

## [S1] Problem

用户要求：**点击下载后创建一个任务**，加一个**下载图标**，点开能看到**每一个下载的速度与进度**，并**添加多线程下载**；「下载页面」即点下载时弹出的那个面板。已确认的决策：弹窗保持为「下载页面」（只加线程选择），另新增任务抽屉；入口放**顶栏右上角图标**，点击从右侧滑出。

现状约束：下载层只有单连接 `curl`，状态按文件名散在两个全局表里（`active` / `dests`），没有速度、没有线程概念，也没有可供面板读取的任务快照；下载状态只在内存里，面板一关就看不到。

## [S2] Design

### 1. 分段下载（`downloader.rs`）

- `parse_probe(head) -> (total, ranges_ok)`：扫 `curl -D -` 的全部响应头，`Content-Range: bytes 0-0/N` 取总长，**只有真的回了 206 才认为支持分段**（光有 `Accept-Ranges` 也可能被忽略）
- `probe()`：`curl -sL -r 0-0 --max-filesize 1 -D - -o <null> --max-time 25`；探测进程登记进 job，取消能立刻杀掉
- `split_ranges(total, n)`：闭区间切段，余数全给最后一段，保证首尾相接、无重叠、无空段；`n` 会被 `min(total)` 与 `MAX_THREADS(8)` 夹住
- 分段条件：`ranges_ok && total >= 4MB && n > 1`；否则单连接直接写目标文件（不做多余的合并拷贝）
- 各段 `curl -L --fail --retry 2 -sS -o <dest>.partN -r start-end <url>`
- 轮询（250ms）：累计各段 `metadata().len()` → 进度 + 指数平滑速度（`speed*0.6 + inst*0.4`）
- 合并：`merge_parts` 按序 `io::copy` 到目标文件 → 删 `.partN`；合并前校验字节总数
- 任一段非零退出 → 立刻 `kill_all` 其余段并报错；取消 → 杀全部子进程 + 删临时文件与半截文件

### 2. 任务注册表与命令

- `TaskInfo { fileName, name, url, status, received, total, percent, speed, threads, path, code, startedAt }`
- `start_download(url, fileName, proxy, downloadDir, threads?, name?)`：登记 job 后**立刻**写一条任务记录（面板马上能看到，不用等探测），随后后台线程做探测+分段
- `list_downloads()`（按 startedAt 倒序）、`clear_download(fileName)`（下载中拒绝）、`clear_finished_downloads()`
- `download-progress` 事件载荷增加 `speed` / `threads`
- **`unregister()`**：成功/失败/取消三条路径都必须摘掉登记，否则同名文件永远「已在下载中」

### 3. 前端

- store：`DlState` 增加 `speed`/`threads`；新增 `downloadTasks`（后端任务表）、`taskPanelOpen`、`activeDownloadCount`、`refreshDownloadTasks()`、`patchDownloadTask()`（事件先到就先建一条最小记录，角标立刻亮）
- `App.vue`：全局监听把进度同步进 `downloadStates` 与 `downloadTasks`；顶栏右上角 + 左侧栏各一个任务入口（带角标）
- `components/DownloadTasks.vue`：右侧滑出抽屉，逐条显示速度/进度/线程/状态 + 取消/移除 + 清空已结束
- `DownloadsView`：弹窗加线程选择与速度显示；`start_download` 抛「已在下载中」时改为与后端任务表对齐而不是标失败
- 设置页「下载」组新增 `downloadThreads`（默认 4）

### 验证边界

- 纯函数单测：切段边界、Range 探测解析、登记摘除、BOM
- 算法独立脚本：本地 Range 服务 + 相同 curl 参数 + sha256 比对
- 实机：4 线程落盘 sha256 一致、无 Range 自动 1 线程、速度与字节增量吻合、角标、取消后无残留

## [S3] Out of Scope

- 断点续传/暂停恢复（本轮只做「取消即清理」）
- 全局并发任务上限与排队调度（当前每个文件独立并发，不做队列）
- BT/ed2k 等非 HTTP 协议；分片校验（不做 per-part hash）
- 代理链的分段（分段与代理叠加时仍走同一个 `--proxy`）

## Tasks

- [x] T1: 分段下载器（探测/切段/并发/合并/失败收摊）+ 速度 + 任务注册表 — acceptance: `cargo test` 全绿；`verify-segments.mjs` sha256 一致 (covers: S2 §1 §2)
- [x] T2: 三个新命令 + `start_download` 扩展 threads/name — acceptance: `cargo check` 零警告 (covers: S2 §2; depends: T1)
- [x] T3: 顶栏任务图标 + 角标 + 右侧抽屉 + 弹窗线程选择 + 设置项 — acceptance: 构建/typecheck PASS (covers: S2 §3; depends: T2)
- [x] T4: 实机验证（4 线程 / 无 Range 回退 / 角标 / 速度 / 取消清理）+ 修「结束任务未摘登记」 — acceptance: 两种路径 sha256 均与源一致、无 `.part*` 残留 (covers: S2; depends: T3)
- [x] T5: `sources.json` BOM 兼容（实测踩到）+ 文档定稿 — acceptance: cargo test 38/38 (covers: S2; depends: T4)
- [x] T6: 取消清理修复（Windows 句柄未释放 → 删除重试）+ 取消后幽灵任务修复（`canceledTasks` 守卫） — acceptance: 取消后 `.part*` 与目标文件均为 0、抽屉无复活任务、可再次下载 (covers: S2; depends: T4)

### 实测数字存档（2026-10-06）

| 指标 | 值 |
|---|---|
| cargo test | **38/38 PASS** |
| 算法脚本 `verify-segments.mjs` | 4 段 / 5MB+12345B / sha256 一致 |
| 40MB 4 线程实机 | sha256 `3ec87c25…` 一致，part 残留 0 |
| 20MB 无 Range 实机 | 1 线程、1.0–1.1 MB/s、sha256 `c6f38437…` 一致 |
| 4 线程速度读数 | 3.0 MB/s（与字节增量吻合） |
| Range 探测 | `206 content-range: bytes 0-0/41943040` |
| 取消清理 | 目标文件与 `.part*` 残留均为 0，无残留 curl 进程 |
| YYL.exe | **5.13 MB**；NSIS setup **2.00 MB**（<20MB 硬指标不变） |
| dist/index.html | 525534 B 自包含 |

复验命令：
`cd src-tauri && cargo test`；`node scripts/verify-segments.mjs`；
`node scripts/serve-testfile.mjs 8899 40 --rate=1024`（另开 8898 加 `--norange`）；
`node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`；
实机：`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` 启动 `YYL.exe`，用 `scripts/cdp-eval.mjs` 读数。
