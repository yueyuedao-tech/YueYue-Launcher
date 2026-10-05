---
feature: settings-topnav
status: designed
updated: 2026-10-05
branch: master
commits: # empty while in progress
---

# 顶栏导航强化 + 设置分区快捷入口 + 双处个性化

## Report

## [S1] Problem

用户要求（对应红圈意图的文字确认，原截图误贴为游戏画面）：
1. 设置相关分区——**下载 / 镜像源 / GitHub / 启动 / 个性化**——移动到顶部导航区域，做成**可点击的快捷入口，点击后跳转到对应分栏**；
2. **默认导航从左侧移到顶部**并明显显示（"原本左边的…移到上面，其他的也移到上面"）；
3. 「个性化」**两处都放**：独立个性化页保留，同时设置页内也增加个性化分区（共享同一份设置状态，天然同步）。

## [S2] Design

### 1. 默认顶部导航

- `settings.navPosition` 默认值 `left` → **`top`**；存量本地存储做一次迁移（无 `navMigrated` 标记则强制置 top 并打标），保证升级后立即生效
- 互换能力保留（个性化页仍可切回左侧）；顶部栏样式强化「明显显示」：主导航项字号/内边距加大、活动态渐变下划线加粗（已有底色+下划线，微调即可）

### 2. 顶栏结构（三段式）

```
[Logo+标题] ……… [主导航×6 居中：首页/实例/下载/Mod/设置/个性化] ……… [设置分区快捷×5]
```

- 右侧快捷组（次级视觉：较小字号、半透明，hover 提亮，与主区竖线分隔）：
  **下载设置 · 镜像源 · GitHub · 启动 · 个性化**
- 点击行为：全部跳转——`下载设置/镜像源/GitHub/启动` → 设置页并激活对应分区 tab；`个性化` → 设置页的「个性化」分区（与 Q2 两处共存一致）
- 新增 store：`settingsTab: 'download'|'mirror'|'github'|'launch'|'personal'`（仅会话态，不持久化），App 提供 `goSettings(tab)`；设置页 `watch settingsTab` 切换激活 tab
- 仅 `navPosition==='top'` 时渲染快捷组（左侧模式由 rail 承载导航，快捷组同样显示——经确认语义为"移到上面"，快捷组常驻顶部栏，与主导航同栏）

### 3. 设置页新增「个性化」分区（两处共放）

- 抽出共享组件 `PersonalControls.vue`（导航位置单选 + 主题 + 强调色 + 遮罩强度 + 置顶 + 关闭行为），`PersonalView` 与 `SettingsView` 的「个性化」tab 共用——同一 `settings` 源，任一处修改两处即时同步
- 设置页分区 tab 变为：**下载 / 镜像与网络 / GitHub / 启动 / 个性化**（5 个）
- 独立个性化页保留不动

### 验证边界

- 构建链 PASS；CDP 实测：新环境（清 localStorage）默认 top、带旧值的存储迁移为 top、右快捷5项点击落到设置正确分区、个性化在设置与独立页双处可见且改强调色两边同步、左侧互换仍可用

## [S3] Out of Scope

- 快捷入口拖拽自定义/增删
- 移动端（≤768px 仍走底部栏）
- Electron 壳

## Tasks

- [ ] T1: navPosition 默认 top + 迁移、顶栏三段式与右侧快捷组、settingsTab 跳转联动、PersonalControls 共享组件与设置页第5分区 — acceptance: 构建通过；上述 CDP 实测项全部通过 (covers: S2)
- [ ] T2: 体积回归 + 独立评审 + 文档定稿 — acceptance: setup≤20MB；评审 PASS；status=delivered (covers: S2; depends: T1)
