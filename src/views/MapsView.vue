<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { effectiveProxy, instances, remoteProxyUrl, selectedInstanceId, settings } from '../store'
import type { MapItem, MapPage } from '../types'

const versions = [
  { value: '', label: '全部版本' },
  { value: '3', label: 'v5 (104)' },
  { value: '4', label: 'v6 (126)' },
  { value: '5', label: 'v7 (135)' },
  { value: '7', label: 'v7.5 (136-146)' },
  { value: '8', label: 'v8a (147-149)' },
  { value: '9', label: 'v8b (150-151)' },
  { value: '10', label: 'v8c (152-154)' },
  { value: '11', label: 'v8 (155+)' },
]
const query = ref('')
const appliedQuery = ref('')
const version = ref('')
const page = ref(0)
const pageInput = ref('1')
const maps = ref<MapItem[]>([])
const hasMore = ref(false)
const loading = ref(false)
const error = ref('')
const installing = ref<number | null>(null)
const installed = ref<Record<number, string>>({})
const installError = ref<Record<number, string>>({})
const previewMap = ref<MapItem | null>(null)
let requestId = 0

const currentName = computed(() => instances.value.find((i) => i.id === selectedInstanceId.value)?.name ?? '')

async function fetchPage(targetPage: number): Promise<MapPage> {
  const result = (await invoke('search_maps', {
    page: targetPage,
    query: appliedQuery.value,
    version: version.value,
    proxy: effectiveProxy.value,
    remoteProxy: settings.remoteProxy,
    cacheMinutes: Math.min(60, Math.max(1, Number(settings.mapCacheMinutes) || 60)),
  })) as MapPage
  return result
}

async function cachePreviewImages(items: MapPage['items']) {
  await Promise.allSettled(items.map(async (item) => {
    if (!item.preview.startsWith('https://')) return
    try {
      const path = await invoke('cache_map_preview', {
        url: item.preview,
        proxy: effectiveProxy.value,
        remoteProxy: settings.remoteProxy,
        cacheMinutes: Math.min(60, Math.max(1, Number(settings.mapCacheMinutes) || 60)),
      }) as string
      item.preview = convertFileSrc(path)
    } catch {
      item.preview = remoteProxyUrl(item.preview)
    }
  }))
}

async function loadMaps() {
  const id = ++requestId
  pageInput.value = String(page.value + 1)
  const targetPage = page.value
  loading.value = true
  error.value = ''
  try {
    const result = await fetchPage(targetPage)
    if (id !== requestId) return
    maps.value = result.items
    hasMore.value = result.hasMore
    void cachePreviewImages(maps.value)
    void prefetchNeighbors(targetPage)
  } catch (e) {
    if (id !== requestId) return
    maps.value = []
    hasMore.value = false
    error.value = String(e)
  } finally {
    if (id === requestId) loading.value = false
  }
}

async function prefetchNeighbors(center: number) {
  // 后台准备后两页；第一页展示后立即把第二、第三页写入后端磁盘缓存。
  const neighbors = [center + 1, center + 2]
  await Promise.allSettled(neighbors.map((p) => fetchPage(p)))
}

function search() {
  appliedQuery.value = query.value.trim()
  page.value = 0
  void loadMaps()
}

function changePage(next: number) {
  if (next < 0) return
  page.value = next
  void loadMaps()
}

function jumpPage() {
  const next = Number.parseInt(pageInput.value, 10)
  if (!Number.isFinite(next) || next < 1) {
    pageInput.value = String(page.value + 1)
    return
  }
  if (next - 1 === page.value) {
    pageInput.value = String(page.value + 1)
    return
  }
  changePage(next - 1)
}

async function install(item: MapItem) {
  if (!selectedInstanceId.value || installing.value !== null) return
  installing.value = item.id
  delete installError.value[item.id]
  try {
    const path = (await invoke('install_map', {
      instanceId: selectedInstanceId.value,
      mapId: item.id,
      proxy: effectiveProxy.value,
      remoteProxy: settings.remoteProxy,
    })) as string
    installed.value[item.id] = path
  } catch (e) {
    installError.value[item.id] = String(e)
  } finally {
    installing.value = null
  }
}

watch(version, () => {
  page.value = 0
  void loadMaps()
})
watch(selectedInstanceId, () => {
  installed.value = {}
  installError.value = {}
})
onMounted(loadMaps)
</script>

<template>
  <section class="page maps-page">
    <div class="toolbar maps-toolbar">
      <div>
        <h1 class="page-title">地图</h1>
      </div>
      <select v-model="selectedInstanceId" class="field maps-instance" aria-label="安装到游戏">
        <option v-for="i in instances" :key="i.id" :value="i.id">{{ i.name }}</option>
        <option v-if="!instances.length" value="" disabled>尚无游戏</option>
      </select>
    </div>

    <div class="maps-filters">
      <input v-model="query" class="field maps-search" placeholder="搜索地图名称或 ID" aria-label="搜索地图" @keydown.enter="search" />
      <select v-model="version" class="field" aria-label="游戏版本">
        <option v-for="v in versions" :key="v.value" :value="v.value">{{ v.label }}</option>
      </select>
      <button class="btn-grad" :disabled="loading" @click="search">搜索</button>
    </div>

    <p v-if="!instances.length" class="maps-note">请先在“游戏”页创建游戏，随后可直接安装地图。</p>
    <p v-if="error" class="maps-error">{{ error }}</p>
    <div v-if="loading" class="empty">正在加载地图…</div>
    <div v-else-if="!maps.length && !error" class="empty">没有找到地图</div>
    <div v-else class="maps-list">
      <article v-for="item in maps" :key="item.id" class="maps-item">
        <img v-if="item.preview.startsWith('https://')" class="maps-preview" :src="item.preview" :alt="item.name" loading="lazy" title="点击放大" @click="previewMap = item" />
        <div v-else class="maps-preview maps-preview--empty" aria-hidden="true">#{{ item.id }}</div>
        <div class="maps-copy">
          <h2>{{ item.name }}</h2>
          <div class="maps-meta">#{{ item.id }} · {{ item.mode }} · {{ item.width }}×{{ item.height }}</div>
          <p v-if="item.desc">{{ item.desc }}</p>
          <div class="maps-tags"><span v-for="tag in item.tags.slice(1, 5)" :key="tag">{{ tag.split('§')[0] }}</span></div>
          <div v-if="installed[item.id]" class="maps-success">已安装到 {{ currentName }}</div>
          <div v-if="installError[item.id]" class="maps-error">{{ installError[item.id] }}</div>
        </div>
        <button class="btn-grad maps-install" :disabled="!selectedInstanceId || installing !== null || !!installed[item.id]" @click="install(item)">
          {{ installing === item.id ? '安装中…' : installed[item.id] ? '已安装' : '下载并安装' }}
        </button>
      </article>
    </div>
    <div class="maps-pages" v-if="!loading && (page > 0 || hasMore)">
      <button class="btn-ghost" :disabled="page === 0" @click="changePage(page - 1)">上一页</button>
      <label class="maps-page-jump">第 <input v-model="pageInput" inputmode="numeric" aria-label="页码" @keydown.enter="jumpPage" @blur="jumpPage" /> 页</label>
      <button class="btn-ghost" :disabled="!hasMore" @click="changePage(page + 1)">下一页</button>
    </div>

    <div v-if="previewMap" class="maps-lightbox" role="dialog" aria-modal="true" @click.self="previewMap = null">
      <button class="maps-lightbox-close" aria-label="关闭预览" @click="previewMap = null">×</button>
      <img :src="previewMap.preview" :alt="previewMap.name" />
      <div>{{ previewMap.name }}</div>
    </div>
  </section>
</template>

<style scoped>
.maps-toolbar { justify-content: space-between; gap: 16px; }
.maps-instance { min-width: 180px; max-width: 260px; }
.maps-filters { display: flex; gap: 10px; margin: 16px 0; }
.maps-search { flex: 1; min-width: 0; }
.maps-filters select { min-width: 150px; }
.maps-filters button { padding: 8px 20px; }
.maps-list { display: grid; gap: 8px; }
.maps-item { display: flex; align-items: center; gap: 14px; padding: 10px; border: 1px solid var(--line); background: var(--card); border-radius: 6px; min-width: 0; }
.maps-preview { width: 100px; height: 76px; object-fit: cover; flex: none; background: var(--panel); border-radius: 4px; }
.maps-preview--empty { display: grid; place-items: center; color: var(--ink-dim); }
.maps-copy { flex: 1; min-width: 0; }
.maps-copy h2 { font-size: 15px; line-height: 1.4; overflow-wrap: anywhere; }
.maps-meta { color: var(--ink-dim); font-size: 12px; margin: 3px 0; }
.maps-copy p { color: var(--ink-dim); font-size: 12px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.maps-tags { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 5px; }
.maps-tags span { font-size: 11px; color: var(--cyan); }
.maps-install { flex: none; width: 88px; min-height: 32px; padding: 6px 8px; font-size: 12px; line-height: 1.25; }
.maps-pages { display: flex; justify-content: center; align-items: center; gap: 16px; padding: 18px 0; }
.maps-page-jump { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; }
.maps-page-jump input { width: 48px; height: 30px; border: 1px solid var(--line); border-radius: 5px; background: var(--card); color: var(--ink); text-align: center; }
.maps-note, .maps-error, .maps-success { font-size: 12px; margin: 6px 0; }
.maps-error { color: #ff8f8f; }
.maps-success { color: #6ee7a8; }
.maps-lightbox { position: fixed; inset: 0; z-index: 100; display: grid; place-items: center; gap: 10px; padding: 36px; background: rgba(0, 0, 0, .82); color: var(--ink); cursor: zoom-out; }
.maps-lightbox img { max-width: min(92vw, 1100px); max-height: 82vh; object-fit: contain; image-rendering: pixelated; border-radius: 6px; box-shadow: 0 12px 48px rgba(0, 0, 0, .5); cursor: default; }
.maps-lightbox-close { position: absolute; top: 14px; right: 18px; width: 38px; height: 38px; font-size: 30px; line-height: 1; color: var(--ink); }
@media (max-width: 600px) {
  .maps-toolbar { align-items: start; flex-wrap: wrap; }
  .maps-instance { max-width: none; width: 100%; }
  .maps-filters { flex-wrap: wrap; }
  .maps-filters select { flex: 1; }
  .maps-item { align-items: start; gap: 9px; }
  .maps-preview { width: 72px; height: 72px; }
  .maps-install { width: 76px; min-height: 38px; white-space: normal; }
}
</style>
