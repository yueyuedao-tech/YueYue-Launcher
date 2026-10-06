<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import { effectiveProxy, instances, selectedInstanceId, settings } from '../store'
import type { ModFile, WorkshopItem } from '../types'

const query = ref('')
const searching = ref(false)
const searchError = ref('')
const results = ref<WorkshopItem[]>([])

const mods = ref<ModFile[]>([])
const modsError = ref('')
const busy = ref(false)

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

onMounted(refreshMods)
// 切换游戏立即刷新列表，避免陈旧列表按新 currentId 误删
watch(currentId, refreshMods)
</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">Mod 管理</h1>
        <p class="page-sub" style="margin-bottom: 0">
          工坊搜索（可浏览）· 游戏 Mod 本地管理
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
      <p style="color: var(--ink-dim); font-size: 12px; padding-bottom: 8px">
        注：工坊内容下载需 Steam 拥有者登录（该下载功能暂缓，登录方案就绪后开放）；当前支持浏览搜索与手动放入的 Mod 管理。
      </p>
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
