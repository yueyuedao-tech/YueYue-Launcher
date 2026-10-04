<script setup lang="ts">
import { computed } from 'vue'
import { store, type ViewId } from './store'
import HomeView from './views/HomeView.vue'
import VersionsView from './views/VersionsView.vue'
import DownloadsView from './views/DownloadsView.vue'
import SettingsView from './views/SettingsView.vue'
import LaunchOverlay from './components/LaunchOverlay.vue'

const views = {
  home: HomeView,
  versions: VersionsView,
  downloads: DownloadsView,
  settings: SettingsView,
} as const

const nav: { id: ViewId; label: string; path: string }[] = [
  { id: 'home', label: '主页', path: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z' },
  { id: 'versions', label: '版本', path: 'M4 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z' },
  { id: 'downloads', label: '下载', path: 'M12 4v10m0 0 4-4m-4 4-4-4M5 19h14' },
  { id: 'settings', label: '设置', path: 'M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7zm7.4-2.6.1-1-.1-1 2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-1.7-1L15 3.4h-4l-.3 2.5c-.6.2-1.2.6-1.7 1l-2.4-1-2 3.4 2 1.6-.1 1 .1 1-2 1.6 2 3.4 2.4-1c.5.4 1.1.8 1.7 1l.3 2.5h4l.3-2.5c.6-.2 1.2-.6 1.7-1l2.4 1 2-3.4z' },
]

const current = computed(() => views[store.view])
</script>

<template>
  <div class="shell">
    <nav class="rail" aria-label="主导航">
      <svg class="logo" viewBox="0 0 48 48" aria-label="星启">
        <rect x="2" y="2" width="44" height="44" rx="12" fill="#1c1d2e" stroke="rgba(255,255,255,.12)" />
        <path
          d="M24 9l4.2 9.6 10.4 1-7.8 6.9 2.3 10.2L24 30.5l-9.1 5.2 2.3-10.2-7.8-6.9 10.4-1z"
          fill="url(#g)"
        />
        <defs>
          <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#ff7db0" />
            <stop offset="1" stop-color="#6ee7f9" />
          </linearGradient>
        </defs>
      </svg>
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
