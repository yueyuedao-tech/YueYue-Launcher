<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount } from 'vue'
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
} from './store'
import type {
  DownloadDonePayload,
  DownloadErrorPayload,
  DownloadProgressPayload,
  LaunchExitPayload,
} from './types'
import HomeView from './views/HomeView.vue'
import InstancesView from './views/InstancesView.vue'
import DownloadsView from './views/DownloadsView.vue'
import ModView from './views/ModView.vue'
import SettingsView from './views/SettingsView.vue'
import PersonalView from './views/PersonalView.vue'
import LaunchOverlay from './components/LaunchOverlay.vue'

const views = {
  home: HomeView,
  instances: InstancesView,
  downloads: DownloadsView,
  mod: ModView,
  settings: SettingsView,
  personal: PersonalView,
} as const

const nav: { id: ViewId; label: string; path: string }[] = [
  { id: 'home', label: '首页', path: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z' },
  { id: 'instances', label: '实例', path: 'M4 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z' },
  { id: 'downloads', label: '下载', path: 'M12 4v10m0 0 4-4m-4 4-4-4M5 19h14' },
  { id: 'mod', label: 'Mod', path: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z' },
  { id: 'settings', label: '设置', path: 'M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7zm7.4-2.6.1-1-.1-1 2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-1.7-1L15 3.4h-4l-.3 2.5c-.6.2-1.2.6-1.7 1l-2.4-1-2 3.4 2 1.6-.1 1 .1 1-2 1.6 2 3.4 2.4-1c.5.4 1.1.8 1.7 1l.3 2.5h4l.3-2.5c.6-.2 1.2-.6 1.7-1l2.4 1 2-3.4z' },
  { id: 'personal', label: '个性化', path: 'M4 8h8M16 8h4M4 16h4M12 16h8M14 5v6M8 13v6' },
]

const current = computed(() => views[store.view])
const unlisteners: UnlistenFn[] = []

function go(id: ViewId) {
  store.view = id
  if (id === 'downloads') ackUpdate()
}

onMounted(async () => {
  loadInstances()
  unlisteners.push(
    await listen<LaunchExitPayload>('launch-exit', () => {
      loadInstances()
    }),
  )
  unlisteners.push(
    await listen<DownloadProgressPayload>('download-progress', (e) => {
      const s = downloadStates[e.payload.fileName]
      if (!s) return
      s.percent = e.payload.percent
      s.received = e.payload.received
      s.total = e.payload.total
    }),
  )
  unlisteners.push(
    await listen<DownloadDonePayload>('download-done', (e) => {
      const s = downloadStates[e.payload.fileName]
      if (!s) return
      s.status = 'done'
      s.percent = 100
      s.path = e.payload.path
    }),
  )
  unlisteners.push(
    await listen<DownloadErrorPayload>('download-error', (e) => {
      const s = downloadStates[e.payload.fileName]
      if (!s) return
      s.status = 'error'
      s.code = e.payload.code
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
    <!-- 顶部居中导航（navPosition=top 时显示；窄屏隐藏，由底部栏接管） -->
    <header v-if="settings.navPosition === 'top'" class="topnav">
      <div class="topnav-brand">
        <img class="logo" src="./assets/logo.png" alt="星启启动器 logo" />
        <span class="topnav-title">星启启动器</span>
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
      <div class="topnav-spacer" aria-hidden="true" />
    </header>

    <div class="shell-body">
      <!-- 左侧图标栏（navPosition=left 时显示；窄屏始终显示为底部栏） -->
      <nav
        class="rail"
        :class="{ 'rail--hidden': settings.navPosition === 'top' }"
        aria-label="主导航"
      >
        <img class="logo" src="./assets/logo.png" alt="星启启动器 logo" />
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
  </div>
</template>
