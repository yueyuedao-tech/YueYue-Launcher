---
feature: nav-layout
status: delivered
updated: 2026-10-05
branch: master
commits: 3716357..c8fb294
---

# 导航布局系统（顶/左互换 + 设置分区 + 6 项导航 + 个性化页）

## Report

**What was built** — 导航布局系统：left/top 双容器互换（个性化页一键切换，localStorage 持久化，reload 保持）、6 项导航（首页/实例/下载/Mod/设置/个性化，两容器共用 go() 含下载页红点已读）、个性化独立页（迁入外观+窗口+导航位置）、设置页三 tab 分区（下载/GitHub/启动）、Mod 占位页。窄屏保持底部栏接管。

**Verification** — 构建链全 PASS；CDP 实测：默认左栏 6 项全可导航（页面标题依次正确）、radio 切顶栏后 topnav 6 项/rail 隐藏、设置 3 tab 互斥同刻仅 1 组可见、navPosition 持久化。独立评审 PASS（0 critical；2 个非关键 CSS 项已随 mod-manager 批次修复：窄屏工具栏输入撑满恢复、顶栏红点移至 ::before 避免与活动下划线冲突）。

**Journey log**
- rail 按钮文字在 title 属性（仅图标），CDP 测试用 textContent 找不到——用 title||textContent 兜底。

## [S1] Problem

当前导航只有左侧图标栏 4 项（首页/实例/下载/设置），个性化设置全部挤在设置页里。用户要求：中间顶部新增一条导航栏、支持**左侧与顶部导航位置互换**、设置页内部加分区 tab、新增 **Mod 导航项**（内容由姊妹功能 mod-manager 填充）、新增**个性化独立页面**。

## [S2] Design

### 决策（Grill 已确认）

- 顶/左互换：导航容器二选一，个性化页一键切换（非双导航常驻）
- 设置页内分区 tab
- 导航 6 项：**首页 / 实例 / 下载 / Mod / 设置 / 个性化**
- 「个性化」独立页；Mod 页第一版内容由 mod-manager 提供，本功能先落占位

### 布局容器

- 设置 `navPosition: 'left' | 'top'`（默认 left，localStorage 持久化）
- **left（现状）**：左侧图标栏（Logo+6 图标）显示，顶部栏隐藏
- **top**：顶部栏显示——结构为「左端 Logo+标题 ｜ 中间居中导航项（图标+文字）」，侧栏整体隐藏；顶部栏高度 ~52px，活动项高亮 + 主题色下划线
- 切换即时生效（CSS/v-if），无动画需求

### 页面迁移

- **新增 `PersonalView`（个性化）**：迁入 外观（主题/强调色/主页遮罩强度）+ 窗口（置顶/关闭行为）+ 新增「导航位置」单选（左侧/顶部）
- **设置页瘦身 + 分区 tab**：保留 下载（代理/目录）、GitHub（前缀/小红点）、启动（默认内存）；页内顶部居中 tab：**下载 / GitHub / 启动**（替代原分组纵向排列，tab 切换显示对应 set-group）
- **ModView 占位**：居中文案「Mod 管理由 mod-manager 功能提供」+ 实例选择提示（正式内容下个功能覆盖）

### 状态与代码组织

- store：`settings.navPosition` + `ViewId` 增 `'personal' | 'mod'`
- App.vue：顶部栏与侧栏按 navPosition 互斥渲染；两处导航共用同一 `go()` 逻辑（含下载页已读清红点）
- 设置页/个性化页仅动视图层，既有 settings 字段不变

### 验证边界

- 构建链 PASS；CDP 实测：6 项导航（两种位置均可点切换页面）、navPosition 持久化（reload 后保持）、设置页3个tab切换、个性化页含导航位置单选且切换后顶栏出现/侧栏隐藏

## [S3] Out of Scope

- Mod 页真实功能（= mod-manager）
- 拖拽排序、图标自定义、动画过渡
- Electron 壳

## Tasks

- [x] T1: 导航容器互换 + 6 项导航 + PersonalView 迁移 + 设置页分区 tab + Mod 占位 — acceptance: 构建通过；CDP 实测两种位置互换、持久化、6项可导航、tab 分区生效 (covers: S2)
