<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { store, type ViewId, loadInstances } from './store'
import type { LaunchExitPayload } from './types'
import HomeView from './views/HomeView.vue'
import InstancesView from './views/InstancesView.vue'
import SettingsView from './views/SettingsView.vue'
import LaunchOverlay from './components/LaunchOverlay.vue'

const views = {
  home: HomeView,
  instances: InstancesView,
  settings: SettingsView,
} as const

const nav: { id: ViewId; label: string; path: string }[] = [
  { id: 'home', label: '主页', path: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z' },
  { id: 'instances', label: '实例', path: 'M4 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z' },
  { id: 'settings', label: '设置', path: 'M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7zm7.4-2.6.1-1-.1-1 2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-1.7-1L15 3.4h-4l-.3 2.5c-.6.2-1.2.6-1.7 1l-2.4-1-2 3.4 2 1.6-.1 1 .1 1-2 1.6 2 3.4 2.4-1c.5.4 1.1.8 1.7 1l.3 2.5h4l.3-2.5c.6-.2 1.2-.6 1.7-1l2.4 1 2-3.4z' },
]

const current = computed(() => views[store.view])
let unexit: UnlistenFn | null = null

onMounted(async () => {
  loadInstances()
  // 进程自然退出后刷新实例列表（运行徽标不残留）
  unexit = await listen<LaunchExitPayload>('launch-exit', () => {
    loadInstances()
  })
})

onBeforeUnmount(() => {
  unexit?.()
})
</script>

<template>
  <div class="shell">
    <nav class="rail" aria-label="主导航">
      <img class="logo" src="./assets/logo.png" alt="星启启动器 logo" />
      <button
        v-for="item in nav"
        :key="item.id"
        class="rail-btn"
        :class="{ active: store.view === item.id }"
        :title="item.label"
        :aria-label="item.label"
        @click="store.view = item.id"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
          <path :d="item.path" />
        </svg>
      </button>
    </nav>

    <main class="main">
      <component :is="current" />
    </main>

    <LaunchOverlay v-if="store.launch.open" />
  </div>
</template>
