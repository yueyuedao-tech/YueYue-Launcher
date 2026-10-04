---
feature: tauri-shell
status: designed
updated: 2026-10-04
branch: master
commits: # empty while in progress
---

# Tauri 2 轻量壳（≤20MB 离线客户端）

## Report

## [S1] Problem

game-launcher 交付的 Electron 版实测：win-unpacked 222.9MB、主程序 exe 180MB——Chromium 静态链接导致体积无法压缩到用户要求的 **20MB**，且启动偏慢。用户明确要求：客户端成品（交付给用户的那一个东西）≤20MB、**运行与安装都不依赖联网**；开发机不受限制。经 Grill 用户选定：换 Tauri 2 壳（Rust + 系统 WebView2，本机已装 154.x），并同意安装 Rust 工具链。现有渲染层 `dist/index.html`（自包含单文件，零网络依赖）与全部 UI 代码原样复用；V1 主进程无业务逻辑，Rust 侧只需实现"开窗 + 加载本地 HTML"。Electron 版完整保留作备用，不删除。

## [S2] Design

### 架构

- **壳**：Tauri 2（`src-tauri/`），Rust release 二进制 + 系统 WebView2 渲染 `dist/index.html`（由 Vite 构建产出，经 `frontendDist` 打进包内，全程 file:// 本地加载，无任何网络请求）。
- **窗口**：1280×800、min 375×560、标题「星启 · 启动器」、背景色 #12121c，与 Electron 版一致。
- **图标**：`build/icon.png`（用户提供素材 400×400）先放大至 1024×1024，用 `tauri icon` 生成全套图标。
- **构建**：`@tauri-apps/cli`；Windows 打 NSIS 安装包 + 目录产物；Linux 目标（AppImage/deb）写入配置，实际构建在 Linux 机执行（与 Electron 版同样的边界）。

### 离线保证

- 包内只有本地文件；`dist/index.html` 通过现有 `check:dist` 外链检查（构建流程内验证）。
- 运行时零网络请求（V1 全 mock、纯本地渲染）。
- Windows 运行前置 = WebView2 运行时（本机已装；Win11/新版 Win10 普遍预装）。**不**内嵌固定版本 WebView2（那会直接超 20MB）；目标机缺失时需联网装运行时或预装——此为平台约束，写入交付说明。
- Linux 运行前置 = webkit2gtk-4.1（主流桌面发行版常见）。

### 验收指标（硬性）

1. **体积**：交付产物 ≤20MB——同时报「NSIS 安装包」与「解包目录合计」两个数，取用户可感知的安装包为主判据，两者都要 <20MB（预期安装包 ~5MB、解包 ~8-12MB）。
2. **离线**：断网环境下（本机禁用网卡后实测或以静态检查替代并说明）双击 exe 窗口正常渲染、四页面与模拟启动可用。
3. **启动速度**：实测「进程启动 → 主窗口句柄出现」耗时，与 Electron 版同法测量对比，出对比数字。
4. **回归**：UI 功能与 Electron 版一致（复用同一 `dist/index.html`，截图抽查主页/启动浮层）。

### 明确不做

- 不删改 Electron 版（保留 `electron/`、`scripts/build-win.cjs` 等，作为备用壳）。
- 不迁移业务 IPC（V1 本来就 mock，无迁移面）。
- 不内嵌 WebView2 运行时、不做应用内自动更新。

## [S3] Out of Scope

- Linux 包的实际构建与运行验证（配置就绪，需 Linux 机器）。
- 目标机缺 WebView2/webkit2gtk 时的自动安装引导。
- Rust 侧新增功能（启动游戏、下载等仍留在渲染层 mock）。

## Tasks

- [ ] T1: 安装 rustup + stable 工具链（国内镜像加速），创建 src-tauri 工程并接入现有 dist — acceptance: `npx tauri dev` 或 release 运行能打开窗口且渲染 dist/index.html (covers: S2)
- [ ] T2: 图标生成与 NSIS/目录打包 — acceptance: `tauri build` 产出安装包与解包目录，两者体积均 <20MB，含 check:dist 离线外链检查 (covers: S2; depends: T1)
- [ ] T3: 启动耗时与功能实测 — acceptance: 给出 Tauri vs Electron「启动→窗口出现」耗时数字；Tauri 版窗口截图证明 UI 完整、模拟启动可用 (covers: S2; depends: T2)
- [ ] T4: 独立评审 + 功能文档定稿 — acceptance: 评审 PASS，文档 status=delivered 并记录体积/耗时实测值 (covers: S2; depends: T3)
