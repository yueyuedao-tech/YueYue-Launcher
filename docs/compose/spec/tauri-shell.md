---
feature: tauri-shell
status: delivered
updated: 2026-10-04
branch: master
commits: 882521b..ceb4a24
---

# Tauri 2 轻量壳（≤20MB 离线客户端）

## Report

**What was built** — 为启动器新增 Tauri 2 轻量壳（`src-tauri/`），Rust 最小窗口壳直接内嵌现有 `dist/index.html`（frontendDist，前端资源打进单二进制）。硬指标全部达成：**NSIS 安装包 1.43MB、release exe 3.34MB、安装态合计约 3.7MB**（评审读 installer.nsi 确认仅装 exe+卸载器），远低于 20MB 目标；`webviewInstallMode: skip` 使安装包不含任何联网安装逻辑，应用零网络请求（check:dist 静态外链检查 PASS）。启动耗时（进程启动→可见窗口）实测 **Tauri 81ms vs Electron 324ms**（各 2 次均值，快约 4 倍）。图标由用户素材经 `tauri icon` 生成全套；release profile 采用 LTO/strip/opt=s/panic=abort。Electron 版完整保留为备用壳。`beforeBuildCommand` 已接线 `npm run build && npm run check:dist`，打包前自动做类型检查与离线检查。

**Verification** — `cargo check --release` PASS；`tauri build --bundles nsis` EXIT 0（前后多次）；`check:dist` PASS（452362B 自包含）；渲染回归：PrintWindow 截图像素分区（title 74,73,90 / art 97,114,136）与 Electron 版（75,76,95 / 102,116,134）一致；模拟启动闭环：PostMessage 投递点击验证「启动」打开浮层（背景亮度 350→98）、「取消」关闭恢复（98→350）双 PASS（证据 `scripts/ov-ok-overlay.png`、`ov-ok-closed.png`）；新二进制冒烟 103ms 出窗 + 渲染 350 PASS。独立评审一轮：2 项 critical（浮层证据缺失、文档未定稿）→ 补证据 + 本轮定稿消解；非关键项（构建接线、未用依赖）已修复，csp 收紧因内联包需 unsafe-inline 暂不做。

**Journey log**
- 本机无 MSVC：winget 路线被用户改为「装 VS BuildTools（WinSDK 26100）+ rustup」，rustc 链接冒烟 `link-ok` 后才开工。
- Tauri 2 的 `webviewInstallMode` 合法值是 `"skip"`（v1 的 `skipInstaller` 会被 schema 拒绝）。
- NSIS 工具下载 GitHub 超时——挂用户代理 `127.0.0.1:7897`（HTTPS_PROXY）后成功。
- 该宿主环境 Node 的 argv 怪癖会弄坏 tauri CLI，须用系统 Node 跑；图像结论一律以像素统计为准。
- 窗口自动化：后台进程抢不到前台（用户全屏游戏在顶层），最终用 PostMessage 投给 `Chrome_RenderWidgetHostHWND` 实现免焦点点击。

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

- [x] T1: 安装 rustup + stable 工具链（国内镜像加速），创建 src-tauri 工程并接入现有 dist — acceptance: `npx tauri dev` 或 release 运行能打开窗口且渲染 dist/index.html (covers: S2)
- [x] T2: 图标生成与 NSIS/目录打包 — acceptance: `tauri build` 产出安装包与解包目录，两者体积均 <20MB，含 check:dist 离线外链检查 (covers: S2; depends: T1)
- [x] T3: 启动耗时与功能实测 — acceptance: 给出 Tauri vs Electron「启动→窗口出现」耗时数字；Tauri 版窗口截图证明 UI 完整、模拟启动可用 (covers: S2; depends: T2)
- [x] T4: 独立评审 + 功能文档定稿 — acceptance: 评审 PASS，文档 status=delivered 并记录体积/耗时实测值 (covers: S2; depends: T3)

### 实测数字存档（2026-10-04）

| 指标 | Tauri | Electron |
|---|---|---|
| NSIS 安装包 | **1.43 MB** | （未打 NSIS，目录包 222.9MB） |
| 解包/单二进制 | **3.34 MB**（前端内嵌，含卸载器约 3.7MB） | 222.9 MB |
| 启动→可见窗口（2 次均值） | **81 ms**（101/61，复测 103） | 324 ms（339/308） |

复验命令：`node scripts/check-singlefile.mjs`；`node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis`（系统 Node + 代理）；启动/浮层验证见 Journey log 所述 PostMessage 脚本逻辑（产物截图在 scripts/ov-ok-*.png，已被 gitignore）。
