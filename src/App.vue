<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  store,
  type ViewId,
  loadInstances,
  downloadStates,
  updateDot,
  checkForUpdates,
  ackUpdate,
  settings,
  loadCentralVersions,
  syncCentralVersions,
  loadCentralIndex,
  refreshDownloadTasks,
  patchDownloadTask,
  appendLaunchLog,
  setLaunchExit,
  loadSystemProxy,
  loadCachedMirrors,
  applyWindowSettings,
  notifyDownloadDone,
} from './store'
import type {
  DownloadDonePayload,
  DownloadErrorPayload,
  DownloadProgressPayload,
  LaunchExitPayload,
  LaunchLogPayload,
} from './types'
import HomeView from './views/HomeView.vue'
import InstancesView from './views/InstancesView.vue'
import DownloadsView from './views/DownloadsView.vue'
import ModView from './views/ModView.vue'
import SettingsView from './views/SettingsView.vue'
import LaunchOverlay from './components/LaunchOverlay.vue'
import DownloadTasks from './components/DownloadTasks.vue'
import LaunchDock from './components/LaunchDock.vue'

const views = {
  home: HomeView,
  instances: InstancesView,
  downloads: DownloadsView,
  mod: ModView,
  settings: SettingsView,
} as const

const ALL_NAV: { id: ViewId; label: string; path: string; devOnly?: boolean }[] = [
  { id: 'home', label: '首页', path: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z' },
  { id: 'instances', label: '游戏', path: 'M4 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z' },
  { id: 'downloads', label: '下载', path: 'M12 4v10m0 0 4-4m-4 4-4-4M5 19h14' },
  // Mod 页还没写完：默认隐藏，只有设置里打开「开发者模式」才出现
  { id: 'mod', label: 'Mod', devOnly: true, path: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z' },
  { id: 'settings', label: '设置', path: 'M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7zm7.4-2.6.1-1-.1-1 2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-1.7-1L15 3.4h-4l-.3 2.5c-.6.2-1.2.6-1.7 1l-2.4-1-2 3.4 2 1.6-.1 1 .1 1-2 1.6 2 3.4 2.4-1c.5.4 1.1.8 1.7 1l.3 2.5h4l.3-2.5c.6-.2 1.2-.6 1.7-1l2.4 1 2-3.4z' },
]

const nav = computed(() => ALL_NAV.filter((n) => !n.devOnly || settings.devMode))

// 开发者模式关掉时若正停在 Mod 页，退回首页，避免留下一个进不去的页面
watch(
  () => settings.devMode,
  (on) => {
    if (!on && store.view === 'mod') store.view = 'home'
  },
)

const current = computed(() => views[store.view])
const unlisteners: UnlistenFn[] = []

function go(id: ViewId) {
  store.view = id
  if (id === 'downloads') ackUpdate()
}

onMounted(async () => {
  // 启动页可在设置里选（首页/游戏/下载/设置）
  if (settings.startView) store.view = settings.startView
  loadInstances()
  // 开机即读本地版本缓存（立刻可渲染），随后后台向服务器索引并按需覆盖；
  // 中心源清单也在这里拉：进下载页时直接有内容，不用再等一次索引。
  void loadCentralVersions().then(() => syncCentralVersions())
  void loadCentralIndex()
  // 后端任务表可能与界面不同步（例如 WebView 刚重载），开机对齐一次
  void refreshDownloadTasks()
  // 窗口/代理/镜像这些依赖窗口与系统状态的，等挂载后再落一次
  void loadSystemProxy()
  void loadCachedMirrors()
  void applyWindowSettings()
  // 启动日志在 App 层统一收：浮层收起后仍需继续写入归档，否则关掉浮层就丢日志
  unlisteners.push(
    await listen<LaunchLogPayload>('launch-log', (e) => {
      const prefix = e.payload.stream === 'err' ? '[err] ' : ''
      appendLaunchLog(e.payload.id, prefix + e.payload.line, e.payload.stream === 'err' ? 'warn' : '')
    }),
  )
  unlisteners.push(
    await listen<LaunchExitPayload>('launch-exit', (e) => {
      // 不自动弹日志：只收进左下角圆圈（玩家看过一次后圆圈自己收起）
      setLaunchExit(e.payload.id, e.payload.code)
      loadInstances()
    }),
  )
  unlisteners.push(
    await listen<DownloadProgressPayload>('download-progress', (e) => {
      const p = e.payload
      const s = downloadStates[p.fileName]
      if (s) {
        s.percent = p.percent
        s.received = p.received
        s.total = p.total
        s.speed = p.speed
        s.threads = p.threads
      }
      patchDownloadTask(p.fileName, {
        status: 'downloading',
        percent: p.percent,
        received: p.received,
        total: p.total,
        speed: p.speed,
        threads: p.threads,
      })
    }),
  )
  unlisteners.push(
    await listen<DownloadDonePayload>('download-done', (e) => {
      const s = downloadStates[e.payload.fileName]
      if (s) {
        s.status = 'done'
        s.percent = 100
        s.path = e.payload.path
      }
      patchDownloadTask(e.payload.fileName, {
        status: 'done',
        percent: 100,
        speed: 0,
        path: e.payload.path,
      })
      void notifyDownloadDone(e.payload.fileName)
    }),
  )
  unlisteners.push(
    await listen<DownloadErrorPayload>('download-error', (e) => {
      const s = downloadStates[e.payload.fileName]
      if (s) {
        s.status = 'error'
        s.code = e.payload.code
      }
      patchDownloadTask(e.payload.fileName, {
        status: 'error',
        code: e.payload.code,
        speed: 0,
      })
    }),
  )
  setTimeout(() => {
    checkForUpdates()
  }, 1500)
})

onBeforeUnmount(() => {
  unlisteners.forEach((u) => u())
})
</script>

<template>
  <div class="shell">
    <!-- 顶部导航（navPosition=top 时显示；窄屏隐藏，由底部栏接管） -->
    <header v-if="settings.navPosition === 'top'" class="topnav">
      <div class="topnav-brand">
        <img class="logo" src="./assets/logo.png" alt="YueYue Launcher logo" />
        <span class="topnav-title">YueYue Launcher</span>
      </div>
      <nav class="topnav-items" aria-label="顶部导航">
        <button
          v-for="item in nav"
          :key="item.id"
          class="topnav-btn"
          :class="{ active: store.view === item.id, 'has-dot': item.id === 'downloads' && updateDot }"
          @click="go(item.id)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <path :d="item.path" />
          </svg>
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </header>

    <div class="shell-body">
      <!-- 左侧图标栏（navPosition=left 时显示；窄屏始终显示为底部栏） -->
      <nav
        class="rail"
        :class="{ 'rail--hidden': settings.navPosition === 'top' }"
        aria-label="主导航"
      >
        <img class="logo" src="./assets/logo.png" alt="YueYue Launcher logo" />
        <button
          v-for="item in nav"
          :key="item.id"
          class="rail-btn"
          :class="{ active: store.view === item.id, 'has-dot': item.id === 'downloads' && updateDot }"
          :title="item.label"
          :aria-label="item.label"
          @click="go(item.id)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <path :d="item.path" />
          </svg>
        </button>
      </nav>

      <main class="main">
        <component :is="current" />
      </main>
    </div>

    <LaunchOverlay v-if="store.launch.open" />
    <DownloadTasks />
    <LaunchDock />
  </div>
</template>
