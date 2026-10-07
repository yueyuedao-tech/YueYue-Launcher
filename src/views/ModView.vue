<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import { effectiveProxy, instances, selectedInstanceId, settings } from '../store'
import type { ModCatalogItem, ModCatalogPage, ModFile, WorkshopItem } from '../types'

const query = ref('')
const searching = ref(false)
const searchError = ref('')
const results = ref<WorkshopItem[]>([])

const mods = ref<ModFile[]>([])
const modsError = ref('')
const busy = ref(false)
const catalog = ref<ModCatalogItem[]>([])
const catalogPage = ref(0)
const catalogTotal = ref(0)
const catalogLoading = ref(false)
const catalogError = ref('')
const modDownloading = ref<string | null>(null)
const modDownloadError = ref<Record<string, string>>({})
const modDownloadNames = ref<Record<string, string>>({})
const unlisteners: UnlistenFn[] = []

const currentId = computed(() => selectedInstanceId.value)
const currentName = computed(
  () => instances.value.find((i) => i.id === currentId.value)?.name ?? '',
)

async function refreshMods() {
  modsError.value = ''
  if (!currentId.value) {
    mods.value = []
    return
  }
  try {
    mods.value = (await invoke('list_mods', { instanceId: currentId.value })) as ModFile[]
  } catch (e) {
    modsError.value = String(e)
  }
}

async function search() {
  void loadCatalog(true)
  if (!query.value.trim()) return
  searching.value = true
  searchError.value = ''
  try {
    results.value = (await invoke('search_workshop', {
      query: query.value.trim(),
      proxy: effectiveProxy.value,
      mirror: settings.workshopMirror,
    })) as WorkshopItem[]
    if (!results.value.length) searchError.value = '没有结果'
  } catch (e) {
    results.value = []
    searchError.value = String(e)
  } finally {
    searching.value = false
  }
}

async function loadCatalog(reset = false) {
  if (reset) catalogPage.value = 0
  catalogLoading.value = true
  catalogError.value = ''
  try {
    const result = (await invoke('list_github_mods', {
      query: query.value.trim(),
      page: catalogPage.value,
      proxy: effectiveProxy.value,
    })) as ModCatalogPage
    catalog.value = result.items
    catalogTotal.value = result.total
  } catch (e) {
    catalog.value = []
    catalogTotal.value = 0
    catalogError.value = String(e)
  } finally {
    catalogLoading.value = false
  }
}

function safeFileName(name: string, repo: string): string {
  const base = name.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^[-_.]+|[-_.]+$/g, '')
  const owner = repo.replace('/', '-')
  const file = `${owner}-${base || 'mod'}`
  return file + (base.toLowerCase().endsWith('.jar') || base.toLowerCase().endsWith('.zip') ? '' : '.jar')
}

async function downloadMod(item: ModCatalogItem) {
  if (!currentId.value || modDownloading.value) return
  modDownloading.value = item.repo
  delete modDownloadError.value[item.repo]
  try {
    const info = (await invoke('resolve_github_mod', { repo: item.repo, proxy: effectiveProxy.value })) as { fileName: string; url: string }
    const dir = (await invoke('mods_dir', { instanceId: currentId.value })) as string
    const fileName = safeFileName(info.fileName, item.repo)
    await invoke('start_download', {
      url: info.url,
      fileName,
      proxy: effectiveProxy.value,
      downloadDir: dir,
      threads: settings.downloadThreads,
      name: item.name,
    })
    modDownloadNames.value[item.repo] = fileName
  } catch (e) {
    modDownloadError.value[item.repo] = String(e)
  } finally {
    modDownloading.value = null
  }
}

async function openModsFolder() {
  if (!currentId.value) return
  try {
    const dir = (await invoke('mods_dir', { instanceId: currentId.value })) as string
    await openExternal(dir)
  } catch (e) {
    modsError.value = String(e)
  }
}

async function removeMod(name: string) {
  if (!currentId.value) return
  if (!window.confirm(`删除 Mod「${name}」？`)) return
  busy.value = true
  try {
    await invoke('delete_mod', { instanceId: currentId.value, name })
    await refreshMods()
  } catch (e) {
    modsError.value = String(e)
  } finally {
    busy.value = false
  }
}

function fmtBytes(n: number): string {
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + ' MB'
  if (n >= 1 << 10) return (n / (1 << 10)).toFixed(0) + ' KB'
  return n + ' B'
}

onMounted(async () => {
  await refreshMods()
  await loadCatalog()
  unlisteners.push(await listen<{ fileName: string }>('download-done', async (event) => {
    if (Object.values(modDownloadNames.value).includes(event.payload.fileName)) await refreshMods()
  }))
})
// 切换游戏立即刷新列表，避免陈旧列表按新 currentId 误删
watch(currentId, refreshMods)
onBeforeUnmount(() => unlisteners.forEach((stop) => stop()))
</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">Mod 管理</h1>
        <p class="page-sub" style="margin-bottom: 0">
          GitHub Mod 清单 · 下载到当前游戏
          <template v-if="effectiveProxy"> · 代理 {{ effectiveProxy }}</template>
          <template v-if="settings.workshopMirror"> · 工坊镜像已配</template>
        </p>
      </div>
      <div style="display: flex; gap: 10px; align-items: center">
        <select v-model="selectedInstanceId" class="field" style="min-width: 200px">
          <option v-for="i in instances" :key="i.id" :value="i.id">{{ i.name }}</option>
          <option v-if="!instances.length" value="" disabled>（无游戏）</option>
        </select>
        <button class="btn-ghost" :disabled="!currentId" @click="refreshMods">刷新列表</button>
      </div>
    </div>

    <p v-if="!instances.length" class="page-sub">请先在「游戏」页创建一个游戏</p>

    <!-- 搜索区 -->
    <div v-if="currentId" class="set-group" style="margin-bottom: 16px">
      <h3>搜索创意工坊（{{ currentName }}）</h3>
      <div class="set-row">
        <div class="label">关键词</div>
        <div class="ctrl" style="flex: 1; max-width: 520px; gap: 8px">
          <input
            v-model="query"
            class="field"
            style="width: 100%"
            placeholder="如：music, ui, content"
            @keydown.enter="search"
          />
          <button class="btn-grad" style="font-size: 14px; padding: 9px 22px" :disabled="searching" @click="search">
            {{ searching ? '搜索中…' : '搜索' }}
          </button>
        </div>
      </div>
      <p v-if="searchError" style="color: #ff7db0; font-size: 13px; padding: 0 0 10px">{{ searchError }}</p>
    </div>

    <div v-if="currentId" class="set-group" style="margin-bottom: 16px">
      <h3>Anuken/MindustryMods（{{ catalogTotal }}）</h3>
      <p v-if="catalogError" class="mod-error">{{ catalogError }}</p>
      <div v-if="catalogLoading" class="empty" style="padding: 20px 0">正在读取 Mod 清单…</div>
      <div v-else class="list" style="margin-top: 8px">
        <div v-for="item in catalog" :key="item.repo" class="row-card">
          <div class="row-icon cy">🧩</div>
          <div class="row-main">
            <div class="name">{{ item.name || item.internalName }}</div>
            <div class="meta">{{ item.repo }} · v{{ item.version || '未知' }} · 最低游戏 {{ item.minGameVersion || '未知' }} · ★{{ item.stars }}</div>
            <div v-if="item.description" class="mod-description">{{ item.description.replace(/\s+/g, ' ').slice(0, 180) }}</div>
            <div v-if="modDownloadError[item.repo]" class="mod-error">{{ modDownloadError[item.repo] }}</div>
          </div>
          <div class="row-side">
            <button class="btn-grad mod-download" :disabled="modDownloading !== null" @click="downloadMod(item)">
              {{ modDownloading === item.repo ? '解析中…' : modDownloadNames[item.repo] ? '已开始' : '下载' }}
            </button>
          </div>
        </div>
      </div>
      <div class="mod-pages" v-if="catalogTotal > 20">
        <button class="btn-ghost" :disabled="catalogPage === 0 || catalogLoading" @click="catalogPage--; loadCatalog()">上一页</button>
        <span>第 {{ catalogPage + 1 }} / {{ Math.ceil(catalogTotal / 20) }} 页</span>
        <button class="btn-ghost" :disabled="(catalogPage + 1) * 20 >= catalogTotal || catalogLoading" @click="catalogPage++; loadCatalog()">下一页</button>
      </div>
    </div>

    <!-- 搜索结果 -->
    <div v-if="results.length" class="list" style="margin-bottom: 18px">
      <div v-for="item in results" :key="item.id" class="row-card">
        <div class="row-icon cy">🧩</div>
        <div class="row-main">
          <div class="name">{{ item.title }}</div>
          <div class="meta">ID {{ item.id }}</div>
        </div>
        <div class="row-side">
          <button class="btn-ghost" @click="openExternal(item.url)">页面</button>
        </div>
      </div>
    </div>

    <!-- 当前游戏已装 Mod -->
    <div class="set-group" v-if="currentId">
      <h3>已安装 Mod（{{ mods.length }}）— {{ currentName }}</h3>
      <p v-if="modsError" style="color: #ff7db0; font-size: 13px; padding: 8px 0">{{ modsError }}</p>
      <div v-if="mods.length" class="list" style="margin-top: 8px">
        <div v-for="m in mods" :key="m.name" class="row-card" style="padding: 10px 14px">
          <div class="row-main">
            <div class="name" style="font-size: 14px; word-break: break-all">{{ m.name }}</div>
            <div class="meta">{{ fmtBytes(m.size) }} · {{ m.mtime }}</div>
          </div>
          <div class="row-side">
            <button class="btn-ghost" style="color: #ff8f8f" :disabled="busy" @click="removeMod(m.name)">删除</button>
          </div>
        </div>
      </div>
      <div v-else class="empty" style="padding: 20px 0">该游戏还没有 Mod（可将 .jar 放入其 mods 目录）</div>
      <div style="padding: 4px 0 12px">
        <button class="btn-ghost" @click="openModsFolder">打开 Mod 文件夹</button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.mod-download { min-width: 64px; padding: 7px 12px; font-size: 12px; }
.mod-description { color: var(--ink-dim); font-size: 12px; margin-top: 4px; line-height: 1.4; }
.mod-error { color: #ff8f8f; font-size: 12px; margin-top: 5px; }
.mod-pages { display: flex; justify-content: center; align-items: center; gap: 16px; padding: 14px 0 4px; }
</style>
