---
feature: mindustry-instances
status: designed
updated: 2026-10-04
branch: master
commits: # empty while in progress
---

# Mindustry 实例管理与真实启动（数据/Java 隔离）

## Report

## [S1] Problem

启动器目前的"启动"是纯前端模拟，且版本/下载页是 Minecraft 风格的 mock。用户要求把启动目标改为 **Mindustry 游戏客户端**，实现「Java 隔离 / 数据隔离」：每个客户端（实例）拥有独立文件夹，文件夹内包含 **json 清单 + 配置文件**，各实例的存档与设置互不可见。源码调研结论（Anuken/Mindustry `DesktopLauncher` + Anuken/Arc `OS.getAppDataDirectoryString`）：Windows 下游戏数据目录 = `env("AppData") + "\Mindustry"`，Linux = `XDG_DATA_HOME`（缺省 `~/.local/share`）。因此**在 spawn Java 子进程时注入自定义 `AppData`/`XDG_DATA_HOME` 环境变量即可整体重定向数据目录**，对游戏零侵入。本机现状：Java 27 在 PATH（另有 JDK 21/25/27）；可启动 jar 有 4 个开发产物（Desktop `Mindustry.jar` 82-83MB），Steam 版位于 `E:\SteamLibrary\steamapps\common\Mindustry`（v1 不支持，见 S3）；`%APPDATA%\Mindustry` 为空。

## [S2] Design

### 决策（Grill 已确认）

1. 后端写在 **Tauri（Rust）** 壳内——保住 1.43MB 安装包；Electron 壳不动、继续作备用。
2. 实例 = 双文件 + 环境变量隔离；每实例可绑定独立 Java。
3. V1 = 完整实例管理 + 真实启动（不含 Steam 版客户端）。

### 实例目录结构

默认根目录：Windows `%APPDATA%\StarlightLauncher\instances\`，Linux `${XDG_DATA_HOME:-~/.local/share}/starlight-launcher/instances/`（V1 固定默认根，不可改）。

```
instances/<实例名>/
├── instance.json        # 清单
├── launch.config.json   # 启动配置
└── data/                # 游戏数据目录（env 注入指向这里）
    └── Mindustry/       # 游戏自建（存档、设置等）
```

**instance.json**：`{ id, name, createdAt, jarPath, javaPath, dataDir }`
- `javaPath`：可执行文件绝对路径，或 `"java"` 表示走 PATH（=每实例 Java 绑定）
- `dataDir`：相对实例根的目录名，固定 `"data"`

**launch.config.json**：`{ isolate: true, memoryMb: 4096, jvmArgs: [], gameArgs: [] }`
- `isolate: true` 时注入 `AppData=<绝对data路径>`（Windows）/ `XDG_DATA_HOME=<绝对data路径>`（Linux）
- 启动命令：`<javaPath> -Xmx<memoryMb>M <jvmArgs...> -jar <jarPath> <gameArgs...>`
- 进程 `cwd` = jar 所在目录（游戏相对路径资源可正常解析；数据隔离由 env 负责）

### Rust 侧（Tauri v2 commands + events）

- `list_instances() -> Vec<InstanceInfo>`：扫描根目录，读双文件，缺失/损坏的条目跳过并标注
- `create_instance(name, jarPath, javaPath, jvmArgs) -> InstanceInfo`：校验名字唯一/jar 存在/java 可执行，创建目录与双文件、`data/`
- `update_instance(id, patch)`：更新清单与配置（含改 jar/java/参数/隔离开关）
- `delete_instance(id)`：递归删除实例目录（前端二次确认）
- `scan_jars() -> Vec<String>`：自动探测——各盘 `steamapps/common/Mindustry` 下的 jar、`Desktop` 下 4 层内 `Mindustry.jar`；去重
- `launch_instance(id)`：spawn 进程（isolate 注入 env、cwd=jar 目录），注册到运行表
- `stop_instance(id)`：杀进程树（Windows `taskkill /T`，Linux 杀进程组）
- 事件 `launch-log { id, line }`、`launch-exit { id, code }`：逐行 stdout/stderr 推给前端
- 仅允许同时运行同一实例一个进程；重复启动返回错误

### 前端改造

- 导航：**主页 / 实例 / 设置**（移除 mock 的"版本列表"与"下载"两页）
- **实例页**：卡片列表（名称、jar、java、隔离状态、运行中徽标）+ 新建表单（名称、jar 下拉=自动扫描结果+手动输入、java 下拉=PATH/java21/25/27+手动、JVM 参数）+ 编辑 + 删除（二次确认）
- **主页**：版本选择器改为**实例选择器**（读真实实例）；"启动"调 `launch_instance`
- **启动浮层复用改造**：日志区接 `launch-log` 实时滚动；进度条改不确定态脉冲动画（真实进度未知）；"取消"= `stop_instance`；收到 `launch-exit` 显示成功/失败（exit code），非零标红
- 设置页保留；与启动相关项（默认内存）后续再与实例配置打通（V1 以实例配置为准）

### 验证边界

- `cargo check` + `npm run build` + `tauri build --bundles nsis` 全绿；体积不回归（仍 <20MB）
- 功能实测（本机真实跑一次）：创建测试实例指向本机 jar → 启动 → 实例 `data/Mindustry` 被游戏创建、浮层出现真实日志 → 停止进程 → 两个实例的 data 相互独立（各建各的目录）
- 桌面 UI 冒烟沿用 PrintWindow/像素法

## [S3] Out of Scope

- Steam 版客户端（`Mindustry.exe`）作为可启动项（env 注入理论可用，未实测不承诺）
- 游戏内 mod 管理、多实例同时游戏的性能/端口冲突处理
- 版本下载、账号、自动更新；mock 下载页不迁移直接移除
- 实例根目录自定义设置、导入导出实例
- Electron 壳接入本功能

## Tasks

- [ ] T1: Rust 实例层——目录结构/双文件读写/创建/列表/更新/删除 + jar 扫描 — acceptance: cargo check 通过；单测或临时命令验证 create/list/roundtrip json 正确 (covers: S2)
- [ ] T2: Rust 启动层——launch/stop/log事件/env隔离注入/cwd — acceptance: cargo check 通过；本机实测启动测试实例产生 launch-log 事件、data/Mindustry 被创建、stop 后进程退出 (covers: S2; depends: T1)
- [ ] T3: 前端实例页与导航改造（列表/新建/编辑/删除/扫描选 jar） — acceptance: 构建通过；界面截图核对四要素齐全，损坏实例条目被跳过标注 (covers: S2; depends: T1)
- [ ] T4: 主页实例选择器 + 启动浮层接真实日志/停止/退出状态 — acceptance: 真实启动走通：浮层滚动日志、停止可用、exit code 展示；像素/截图证据 (covers: S2; depends: T2; T3)
- [ ] T5: 双实例数据隔离实测 + 体积回归 + 独立评审 + 文档定稿 — acceptance: 两个实例各自 data/Mindustry 独立生成互不串；setup≤20MB；评审 PASS；status=delivered (covers: S2; depends: T4)
