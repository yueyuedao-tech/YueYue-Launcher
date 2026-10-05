---
feature: mod-manager
status: designed
updated: 2026-10-05
branch: master
commits: # empty while in progress
---

# Mod 管理（Steam 创意工坊 + steamcmd + 实例级安装 + 统一镜像设置）

## Report

## [S1] Problem

用户要求：**每个实例可以下载 mod**，来源为 Mindustry Steam 创意工坊（`steamcommunity.com/app/1127400/workshop/`，本机实测直连超时、代理 200）；**通过 steamcmd 查询与下载**工坊物品；**镜像与代理集中到同一组设置**（mod、实例 jar、通用下载共用），下载优先级 = **镜像列表优先 → 本地代理 → 直连**。

## [S2] Design

### 范围修订（2026-10-05，用户决策）

实测确认：Mindustry 工坊 depot 对匿名用户返回 **Missing decryption key**（需拥有者登录；文章与日志双证）。用户决策：**取消 steamcmd 下载功能，保留已做的改动**——本功能实际交付范围调整为：

- ✅ 保留：工坊搜索（浏览/跳转页面）、实例 Mod 本地管理（列表/打开目录/删除）、统一镜像设置组（代理/GitHub 前缀/工坊镜像）、全部导航布局改动
- ❌ 移除：steamcmd 引导与下载管线（ensure/download/stop、steamcmd 包镜像设置项、相关事件与前端下载状态机）；Mod 页结果卡仅保留「页面」按钮，页面注明下载功能暂缓待登录方案
- ⏸ 延后：Steam 拥有者交互登录（用户将另行配置后再开放下载）

### 环境事实（已侦察）

- 本机无 steamcmd → 启动器负责引导安装
- `steamcmd.zip` 官方源（774KB，代理实测 200）：`https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip`
- steamcommunity 页面：直连超时、经代理 200 → 必须走镜像/代理链
- steamcmd 首次运行会自更新（一次性，约 1 分钟，需网络）

### 统一设置组「镜像与网络」（用户要求"扔到一起"）

设置页新增该分组，集中迁移+新增：

| 字段 | 原位置 | 用途 |
|---|---|---|
| 下载代理 proxy | 下载组（迁入） | 所有 curl 的 --proxy |
| GitHub 加速前缀 githubPrefix | GitHub 组（迁入） | jar/Atom |
| steamcmd 包镜像 steamCmdMirror | 新增 | steamcmd.zip 下载前缀（空=官方 CDN） |
| 工坊镜像 workshopMirror | 新增 | steamcommunity 页面前缀（空=走代理） |

**优先级链**（每个下载类资源）：`mirror 非空 → 用镜像；否则 proxy 非空 → 用代理；否则直连`。GitHub 组保留小红点开关，下载组保留下载目录。

### steamcmd 引导（零新增 Rust 依赖）

- 数据根：`%APPDATA%\StarlightLauncher\steamcmd\`
- `ensure_steamcmd(proxy, mirror) -> Result<path>`：
  1. 已有 `steamcmd/steamcmd.exe` 且可执行 → 直接返回
  2. 否则 curl 下载 `{mirror|官方}installer/steamcmd.zip` 到数据根 → 系统 `tar -xf` 解压（Win10+ bsdtar 支持 zip；Linux 需 tar 支持 zip，否则报错提示手动放置）
  3. 下载/解压失败 → 明确错误（含网络建议）
- 首次实际调用 steamcmd 时它会自更新（见验证边界）

### 工坊查询（HTML 解析，免 API Key）

- `search_workshop(query, proxy, mirror) -> Vec<WorkshopItem { id, title, url }>`
  - URL：`{mirror|直达}steamcommunity.com/workshop/browse/?appid=1127400&searchtext=<urlencoded>&browsesort=trend&days=90`（仅 http(s) 前缀拼接，query 做百分号编码——零依赖手写编码器）
  - curl（按优先级链）→ 解析 `<a class="workshopItemTitle" ... href="...sharedfile_<id>...">标题</a>` 配对，最多 20 条；解析 0 条 → Err（提示镜像/代理）
- UI：Mod 页实例选择器（复用 selectedInstanceId）+ 搜索框 + 结果卡（标题/ID/[下载到当前实例]）

### 下载到实例

- `download_mod(instanceId, fileId, proxy, mirror)`：
  1. 校验实例存在；创建 `instances/<id>/data/Mindustry/mods/`
  2. ensure_steamcmd
  3. spawn `steamcmd.exe +login anonymous +workshop_download_item 1127400 <fileId> +quit`（cwd=steamcmd 根），stdout/stderr 逐行 emit `mod-log {instanceId, line}`（复用启动浮层同款监听模式）
  4. 等退出：定位 `steamcmd/steamapps/workshop/content/1127400/<fileId>/` 目录存在 → **复制**其中文件到目标 mods/ → emit `mod-done {instanceId, fileId, ok, files}`；steamcmd 失败（非0或无产物）→ emit `mod-done ok=false`
  5. `stop_mod()` 杀 steamcmd 进程树（Windows taskkill /T，Unix 进程组）
  - 运行表：单实例单任务（同 instanceId 并发拒绝；不同实例可并行）
- steamcmd 网络直连 Steam CDN 国内通常可用；steamcommunity 查询必须走链——两者独立配置

### Mod 页操作（当前实例）

- `list_mods(instanceId) -> Vec<ModFile { name, size, mtime }>`：读 `data/Mindustry/mods/`（无目录返回空）
- 打开文件夹：`tauri-plugin-shell` 的 `open`（已具备）打开该目录（前端拿路径——需要后端给绝对路径：`mods_dir(instanceId) -> String` 命令）
- 删除 mod：`delete_mod(instanceId, name)`（name 白名单校验：无分隔符/`..`，必须是 mods 目录直接子文件）
- 下载进行中：结果卡显示进度区（steamcmd 日志行数/最新一行），取消按钮 → stop_mod

### 验证边界

- cargo test：URL 编码器、工坊 HTML 样例解析、mod 文件名校验
- 实机（CDP）：ensure_steamcmd 引导（zip 下载+解压+exe 存在）→ 搜索返回真实工坊条目（经代理）→ 选一条下载到测试实例 → mods/ 出现文件、mod-done ok → list_mods 可见 → 删除一项 → 停止/取消路径
- 首次 steamcmd 自更新耗时 ≤2 分钟内完成（记录实测）
- 体积不回归（无新 Rust 依赖）<20MB；UI 冒烟：6 导航、Mod 页搜索/下载/列表三态

## [S3] Out of Scope

- 创意工坊分页浏览/收藏/评分、Mod 依赖解析与冲突检测
- 非 1127400 的其他游戏工坊
- Linux 实机验证（随 Linux 构建）；steamcmd 在 Linux 需单独二进制（配置就绪后补）
- 启用/禁用开关（后续迭代）

## Tasks

- [x] T1: 工坊搜索（HTML 解析新版 Steam 结构 + urlencode + 单测） — acceptance: cargo test 通过；实机真实搜索返回 15 条 (covers: S2)
- [x] T2: steamcmd 下载管线 — **按用户决策取消移除**（引导/下载/停止/事件/设置项全部摘除，改动保留于 git 历史 20b1466..内） — acceptance: n/a (covers: 范围修订)
- [x] T3: 实例 Mod 本地管理（list_mods/mods_dir/delete_mod）+ 前端 Mod 页（搜索浏览、本地列表、打开目录、删除、暂缓说明）+ 镜像与网络设置组 — acceptance: 构建通过；实机 DOM 冒烟：15 结果、0 下载按钮、暂缓说明在、镜像组无 steamcmd 行 (covers: S2; depends: T1)
- [x] T4: 实机验证 + 体积 + 独立评审 + 文档定稿 — acceptance: 搜索/列表/删除链路实测；setup≤20MB；评审 PASS；status=delivered (covers: S2; depends: T3)

### 实测存档（2026-10-05）

| 指标 | 值 |
|---|---|
| 工坊搜索（代理） | 15 条，首条 subdustry (subnautica) id=3776239024 |
| steamcmd 引导 | 曾实测成功（zip→解压→自更新→连接 Steam OK）；物品下载因 Missing decryption key 失败 → 功能按决策取消 |
| 裁剪后体积 | setup 1.94MB / exe 4.93MB |
| 移除验证 | 结果卡仅 [页面]；设置镜像组仅 代理/GitHub前缀/工坊镜像 |
