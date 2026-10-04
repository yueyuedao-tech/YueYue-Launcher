---
feature: game-launcher
status: designed
updated: 2026-10-03
branch: master
commits: # empty while in progress
---

# PCL 风格二次元游戏启动器

## Report

## [S1] Problem

用户需要一个同时支持 Windows 与 Linux 的桌面 app：游戏启动器，界面模仿 PCL2（Plain Craft Launcher 2），视觉走二次元风格。第一版只要求"界面 + 模拟启动"——完整的启动器 UI、mock 数据、点击启动后的模拟进度与日志动画；不真实拉起 Minecraft。目标目录 `C:\Users\43551\Desktop\app`（全新空目录，已在其中 `git init`，分支 `master`）。

## [S2] Design

### 技术栈与架构

- **桌面壳**：Electron（主进程 Node.js/TypeScript），创建窗口、加载渲染产物，负责 app 生命周期。V1 主进程不包含启动游戏等业务 IPC——模拟启动完全在渲染进程内用定时器完成。
- **界面**：Vue 3 + TypeScript + Vite；`vite-plugin-singlefile` 将 JS/CSS 全部内联，构建产物为**自包含的单文件 `dist/index.html`**（无任何外部 script/link/网络依赖），移动端自适应（≤768px 侧边栏切换为底部导航）。该文件既被 Electron 加载，也可直接用浏览器/手机模拟器打开预览。
- **打包**：electron-builder 配置声明 Windows（NSIS）与 Linux（AppImage）目标；本机只验证 Windows 目录构建，Linux 构建留待 Linux 环境。
- **图像资源**：二次元主视觉背景素材——原计划用图像生成工具产出，实际因生成服务返回 403（会员未开通）、经用户确认改为从免授权站点下载（Pixabay Content License，免费商用无需署名；来源 URL 与许可记录见 `assets/CREDITS.txt`），存于 `src/assets/`（Vite 引用需要），构建时内联进单文件，不引用远程 URL。应用图标与界面图形为 SVG 自绘。

### 页面结构（PCL2 式布局）

左侧竖向图标导航栏 + 右侧内容区；窗口默认 1280×800，可缩放。

1. **主页**：二次元主视觉背景、当前版本选择器（下拉，mock 版本）、大号"启动"按钮、公告/新闻卡片（mock）。
2. **版本列表**：已安装版本的卡片列表（名称、类型、上次启动时间、大小），支持按名称过滤。
3. **下载**：可下载版本/整合包的 mock 列表，每项带"下载"按钮，点击后展示模拟下载进度。
4. **设置**：分组设置项（外观、启动参数、内存分配等 mock），控件可交互，选择写入 `localStorage` 持久化。

### 模拟启动流程

点击"启动"→ 弹出启动进度浮层：进度条推进 + 分阶段日志逐行滚动（校验文件 → 解析版本 → 下载资源 → 启动 Java → 进入游戏），约 6~8 秒后进入"启动成功"状态并可关闭；提供"取消"。全程无真实进程。

### 验证边界

- `npm run build` 成功且 `dist/index.html` 自包含（脚本检查：不含 `http(s)://` 外链资源与外部 `<script src>`）。
- `npm start` 在本机（Windows）拉起 Electron 窗口。
- 页面截图人工核对布局与二次元视觉；移动端宽度自适应核对。

## [S3] Out of Scope

- 真实 Minecraft 启动（Java 检测、版本 JSON 解析、进程拉起、日志回传）。
- 真实下载（版本 jar、Mod、整合包）、Mod 管理、账号/皮肤系统。
- 自动更新、崩溃上报、多语言（界面语言仅中文）。
- 在本机上产出 Linux 安装包（只提供配置与代码，实际构建需 Linux 环境）。

## Tasks

- [ ] T1: 搭建 Vite + Vue3 + TS 工程并接入 vite-plugin-singlefile — acceptance: `npm run build` 产出自包含 `dist/index.html`，通过外链资源脚本检查 (covers: S2)
- [ ] T2: Electron 主进程加载构建产物 — acceptance: Windows 上 `npm start` 打开 1280×800 窗口并正确渲染界面 (covers: S2; depends: T1)
- [ ] T3: 主布局与导航（侧边栏图标导航、四个页面路由骨架、≤768px 底部导航） — acceptance: 四个页面可点击互相切换，窄屏下布局不破 (covers: S2; depends: T1)
- [ ] T4: 主页（主视觉背景、版本选择器、启动按钮、公告卡片，mock 数据） — acceptance: 版本下拉可展开选择，公告卡片渲染 mock 数据 (covers: S2; depends: T3)
- [ ] T5: 版本列表页与下载页（mock 卡片列表、名称过滤、模拟下载进度） — acceptance: 过滤即时生效；点击下载出现进度并完成 (covers: S2; depends: T3)
- [ ] T6: 设置页（分组控件可交互，选择持久化到 localStorage） — acceptance: 刷新/重启后设置值保持 (covers: S2; depends: T3)
- [ ] T7: 模拟启动浮层（进度条 + 分阶段日志滚动 + 成功态 + 取消） — acceptance: 点击启动完整走完模拟流程，日志逐行出现，可中途取消 (covers: S2; depends: T4)
- [ ] T8: 二次元视觉打磨（接入主视觉背景素材，统一配色/图标/圆角卡片风格） — acceptance: 各页面视觉统一，背景图为本地免授权素材（来源与许可见 assets/CREDITS.txt），无水印乱码 (covers: S2; depends: T4)
- [ ] T9: electron-builder 配置（win: nsis / linux: AppImage） — acceptance: 配置文件含两平台目标，Windows 上 `--dir` 目录构建成功 (covers: S2; depends: T2)
