<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import {
  centralVersions,
  downloadStates,
  instances,
  loadInstances,
  settings,
  store,
  versionSyncing,
  type DlState,
} from '../store'
import type {
  CentralAsset,
  CentralIndex,
  CentralItem,
  CentralVersion,
  DownloadDonePayload,
  DownloadErrorPayload,
  GithubVersion,
  SourceItem,
} from '../types'

const sources = ref<SourceItem[]>([])
const central = ref<CentralItem[]>([])
const centralMeta = ref({ source: '', note: '' })
const centralLoading = ref(false)
const subsError = ref('')
const cardErrors = reactive<Record<string, string>>({})
const busy = ref(false)

// 分组折叠状态（初始取组内任一源的 collapsed）
const groupOpen = reactive<Record<string, boolean>>({})
// 本地 github 源的展开状态与版本缓存
const expanded = reactive<Record<string, boolean>>({})
const versions = reactive<
  Record<string, { loading: boolean; error: string; items: GithubVersion[] }>
>({})
/** 本地仓库源：最新一条常显，其余默认收起（中心源的版本全部展开） */
const showAllVersions = reactive<Record<string, boolean>>({})

function shownVersions(id: string): GithubVersion[] {
  const all = versions[id]?.items ?? []
  return showAllVersions[id] ? all : all.slice(0, 1)
}

/** 中心源版本：来自本地缓存，最新版独立置顶，其余默认折叠 */
function centralVersionsOf(id: string): CentralVersion[] {
  return centralVersions.value?.sources?.[id] ?? []
}

/** 该源本次拉取失败的原因（没有数据时才显示，方便定位限流/断网） */
function centralErrorOf(id: string): string {
  return centralVersions.value?.errors?.[id] ?? ''
}

/** 本地源表分组（中心索引单独成区，不再混排、也没有「中心 · 」前缀） */
const groups = computed(() => {
  const order: string[] = []
  const map: Record<string, SourceItem[]> = {}
  for (const s of sources.value) {
    if (!map[s.group]) {
      map[s.group] = []
      order.push(s.group)
    }
    map[s.group].push(s)
  }
  return order.map((g) => ({ name: g, items: map[g] }))
})

function fileNameForDirect(item: SourceItem): string {
  const base = item.name.replace(/[\\/:*?"<>|]/g, '_').trim() || 'download'
  try {
    const path = new URL(item.url).pathname
    const m = path.match(/\.[A-Za-z0-9]{1,7}$/)
    return base + (m ? m[0] : '')
  } catch {
    return base
  }
}

function fileNameForVersion(s: SourceItem, v: GithubVersion): string {
  const tail = s.repo.split('/').pop() ?? 'repo'
  return `${tail}-${v.tag}.jar`.replace(/[\\/:*?"<>|]/g, '_')
}

function applyPrefix(url: string, force: boolean): string {
  const p = settings.githubPrefix.trim()
  if (!p || !url.startsWith('https://github.com/')) return url
  if (!force && url.startsWith(p)) return url
  if (url.startsWith(p)) return url
  return p + url
}

function instanceNameFor(raw: string): string {
  let base = raw.replace(/[\\/:*?"<>|]/g, '_').trim().slice(0, 36) || '游戏'
  if (instances.value.some((i) => i.id === base)) {
    let n = 2
    while (instances.value.some((i) => i.id === `${base}-${n}`)) n++
    base = `${base}-${n}`
  }
  return base
}

function stateOf(fileName: string): DlState | undefined {
  return downloadStates[fileName]
}

/** 资产文件名太长，只留最后两段（如 …-Desktop.jar → Desktop.jar） */
function shortAsset(name: string): string {
  const p = name.split('-')
  return p.length >= 3 ? p.slice(-2).join('-') : name
}

async function loadSources() {
  try {
    sources.value = (await invoke('list_sources')) as SourceItem[]
    for (const g of groups.value) {
      if (!(g.name in groupOpen)) {
        // 组头初始展开，除非组内全部源都是 collapsed
        groupOpen[g.name] = g.items.some((s) => !s.collapsed)
      }
    }
  } catch (e) {
    subsError.value = String(e)
  }
}

/** 查询中心化服务器索引；失败由后端降级为内置索引，这里只负责提示 */
async function loadCentral() {
  centralLoading.value = true
  try {
    const r = (await invoke('fetch_central_index', {
      base: settings.centralServer.trim(),
      proxy: settings.proxy,
    })) as CentralIndex
    central.value = r.items
    centralMeta.value = { source: r.source, note: r.note }
  } catch (e) {
    centralMeta.value = { source: 'builtin', note: String(e) }
    central.value = []
  } finally {
    centralLoading.value = false
  }
}

async function persist(): Promise<boolean> {
  try {
    sources.value = (await invoke('save_sources', { items: sources.value })) as SourceItem[]
    subsError.value = ''
    return true
  } catch (e) {
    subsError.value = String(e)
    await loadSources()
    return false
  }
}

async function toggleGroup(name: string) {
  groupOpen[name] = !groupOpen[name]
  for (const s of sources.value.filter((x) => x.group === name)) {
    s.collapsed = !groupOpen[name]
  }
  await persist()
}

async function removeSource(item: SourceItem) {
  if (!window.confirm(`删除源「${item.name}」？`)) return
  sources.value = sources.value.filter((s) => s.id !== item.id)
  await persist()
}

async function fetchVersions(s: SourceItem) {
  expanded[s.id] = !expanded[s.id]
  if (!expanded[s.id]) return
  showAllVersions[s.id] = false
  if (versions[s.id]?.items.length || versions[s.id]?.loading) return
  versions[s.id] = { loading: true, error: '', items: [] }
  try {
    const items = (await invoke('fetch_repo_versions', {
      repo: s.repo,
      asset: s.asset,
      proxy: settings.proxy,
      prefix: settings.githubPrefix,
    })) as GithubVersion[]
    versions[s.id] = { loading: false, error: '', items }
  } catch (e) {
    versions[s.id] = { loading: false, error: String(e), items: [] }
  }
}

async function startUrl(url: string, fileName: string, mirrorRetry: boolean, errKey: string) {
  cardErrors[errKey] = ''
  downloadStates[fileName] = {
    status: 'downloading',
    percent: 0,
    received: 0,
    total: 0,
  }
  try {
    await invoke('start_download', {
      url: applyPrefix(url, mirrorRetry),
      fileName,
      proxy: settings.proxy,
      downloadDir: settings.downloadDir,
    })
  } catch (e) {
    downloadStates[fileName].status = 'error'
    cardErrors[errKey] = String(e)
  }
}

function startDirect(item: SourceItem) {
  void startUrl(item.url, fileNameForDirect(item), false, item.id)
}

function startVersion(s: SourceItem, v: GithubVersion) {
  void startUrl(v.jarUrl, fileNameForVersion(s, v), false, s.id + ':' + v.tag)
}

/** 镜像重试：前缀为空时给出去设置页的提示，而非静默退化为普通重试 */
function mirrorRetry(url: string, fileName: string, errKey: string) {
  if (!settings.githubPrefix.trim()) {
    cardErrors[errKey] = '请先在设置页「镜像与网络」填写加速前缀，再使用镜像重试'
    return
  }
  void startUrl(url, fileName, true, errKey)
}

async function cancel(fileName: string, errKey: string) {
  try {
    await invoke('stop_download', { fileName })
    // 成功取消 → 复位为可再次下载（后端 stop 不发事件，状态在这里归零）
    delete downloadStates[fileName]
    delete cardErrors[errKey]
  } catch (e) {
    const msg = String(e)
    if (msg.includes('未在下载')) {
      // 后端已无此任务：要么刚完成（done 事件将至/已至，listener 见无条目会忽略）
      // 要么从未启动——都应把卡住的 downloading 复位
      delete downloadStates[fileName]
      delete cardErrors[errKey]
    } else {
      cardErrors[errKey] = msg
    }
  }
}

async function createInstanceFrom(fileName: string, suggested: string, errKey: string) {
  const st = stateOf(fileName)
  if (!st?.path) return
  busy.value = true
  cardErrors[errKey] = ''
  try {
    await invoke('create_instance', {
      name: instanceNameFor(suggested),
      jarPath: st.path,
      javaPath: 'java',
      jvmArgs: [],
      memoryMb: settings.memory,
      isolate: true,
    })
    await loadInstances()
    store.view = 'instances'
  } catch (e) {
    cardErrors[errKey] = String(e)
  } finally {
    busy.value = false
  }
}

async function openPage(url: string) {
  try {
    await openExternal(url)
  } catch (e) {
    subsError.value = String(e)
  }
}

function fmtBytes(n: number): string {
  if (n >= 1 << 30) return (n / (1 << 30)).toFixed(2) + ' GB'
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + ' MB'
  if (n >= 1 << 10) return (n / (1 << 10)).toFixed(0) + ' KB'
  return n + ' B'
}

function baseName(p: string) {
  return p.split(/[\\/]/).pop() ?? p
}

/** 旧版本默认折叠（最新版始终独立显示在最上面） */
const showOld = reactive<Record<string, boolean>>({})

type DlState2 = {
  sourceName: string
  version: string
  url: string
  fileName: string
  name: string
  downloadDir: string
  memoryMb: number
  isolate: boolean
  javaPath: string
  jvmArgs: string
  state: 'form' | 'downloading' | 'creating' | 'done' | 'error'
  error: string
}

/** 点「下载」直接弹出创建面板；所有字段留空即走默认值 */
const dlPanel = ref<DlState2 | null>(null)

function defaultFileName(sourceName: string, version: string, url: string): string {
  const ext = url.match(/\.[A-Za-z0-9]{1,7}$/)?.[0] ?? '.jar'
  const raw = `${sourceName}-${version}`.replace(/[\\/:*?"<>|]+/g, '_').trim()
  return (raw || 'download').slice(0, 80) + ext
}

function askDownload(c: CentralItem, v: CentralVersion, a: CentralAsset) {
  dlPanel.value = {
    sourceName: c.name,
    version: v.tag,
    url: a.url,
    fileName: defaultFileName(c.name, v.tag, a.url),
    name: `${c.name} ${v.tag}`.slice(0, 40),
    downloadDir: settings.downloadDir,
    memoryMb: settings.memory,
    isolate: settings.saveIsolation,
    javaPath: 'java',
    jvmArgs: '',
    state: 'form',
    error: '',
  }
}

function closePanel() {
  const s = dlPanel.value?.state
  if (s === 'downloading' || s === 'creating') return
  dlPanel.value = null
}

async function confirmDownload() {
  const p = dlPanel.value
  if (!p) return
  p.state = 'downloading'
  p.error = ''
  try {
    await invoke('start_download', {
      url: p.url,
      fileName: p.fileName,
      proxy: settings.proxy,
      downloadDir: p.downloadDir,
    })
  } catch (e) {
    p.state = 'error'
    p.error = String(e)
  }
}

/** 下载完成后自动用面板里选的隔离/内存/路径创建游戏 */
async function createFromPanel(path: string) {
  const p = dlPanel.value
  if (!p) return
  p.state = 'creating'
  p.error = ''
  try {
    await invoke('create_instance', {
      name: p.name,
      jarPath: path,
      javaPath: p.javaPath.trim() || 'java',
      jvmArgs: p.jvmArgs.split(/\s+/).filter(Boolean),
      memoryMb: p.memoryMb,
      isolate: p.isolate,
    })
    p.state = 'done'
    await loadInstances()
  } catch (e) {
    p.state = 'error'
    p.error = String(e)
  }
}

const unlisteners: UnlistenFn[] = []

onMounted(async () => {
  void loadCentral()
  void loadSources()
  unlisteners.push(
    await listen<DownloadDonePayload>('download-done', (e) => {
      const p = dlPanel.value
      if (p && e.payload.fileName === p.fileName) void createFromPanel(e.payload.path)
    }),
    await listen<DownloadErrorPayload>('download-error', (e) => {
      const p = dlPanel.value
      if (p && e.payload.fileName === p.fileName) {
        p.state = 'error'
        p.error = `下载失败 code=${e.payload.code}`
      }
    }),
  )
})

onBeforeUnmount(() => unlisteners.forEach((u) => u()))

</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">下载中心</h1>
      </div>
    </div>

    <p v-if="subsError" style="color: #ff7db0; font-size: 13px; margin-bottom: 12px">{{ subsError }}</p>
    <p v-if="centralMeta.note" style="color: var(--amber); font-size: 12px; margin-bottom: 12px">
      {{ centralMeta.note }}
    </p>

    <!-- 中心索引：logo/标签/版本均由中心服务器下发 -->
    <div v-if="centralLoading" class="empty">正在拉取中心索引…</div>
    <template v-else>
      <div v-for="c in central" :key="c.id" class="row-card" style="flex-direction: column; align-items: stretch; gap: 0; margin-bottom: 14px">
        <div style="display: flex; gap: 14px; align-items: flex-start">
          <div class="row-icon cy" style="overflow: hidden">
            <img v-if="c.logo" :src="c.logo" alt="" style="width:100%;height:100%;object-fit:cover" @error="($event.target as HTMLImageElement).style.display='none'" />
            <template v-else>{{ c.name.slice(0, 1) }}</template>
          </div>
          <div class="row-main">
            <div class="name">
              {{ c.name }}
              <span v-for="t in c.tags" :key="t" class="tag" style="margin-left: 6px">{{ t }}</span>
            </div>
            <div class="meta" style="word-break: break-all">{{ c.url }}</div>
            <div v-if="c.note" class="meta">{{ c.note }}</div>
          </div>
        </div>

        <div class="vlist">
          <div v-if="versionSyncing && !centralVersionsOf(c.id).length" class="meta" style="padding: 8px 0">
            正在扫描版本…
          </div>
          <div v-else-if="!centralVersionsOf(c.id).length" class="meta" style="padding: 8px 0; color: #ff8f8f">
            {{ centralErrorOf(c.id) || '本地暂无版本缓存，索引完成后自动填充' }}
          </div>
          <!-- 最新版独立置顶 -->
          <div
            v-if="centralVersionsOf(c.id).length"
            class="row-card row-card--latest"
            style="padding: 13px 15px"
          >
            <div class="row-main">
              <div class="name" style="font-size: 14.5px">
                {{ centralVersionsOf(c.id)[0].tag }}
                <span class="tag" style="margin-left: 6px; background: rgba(255,125,176,.16); color: var(--pink)">最新</span>
                <span
                  v-if="centralVersionsOf(c.id)[0].dropped"
                  class="tag"
                  style="margin-left: 6px; background: rgba(255,255,255,.07); color: var(--ink-dim)"
                >
                  已滤除 {{ centralVersionsOf(c.id)[0].dropped }} 个服务端
                </span>
              </div>
              <div class="meta">
                {{ centralVersionsOf(c.id)[0].title }}{{ centralVersionsOf(c.id)[0].date ? ' · ' + centralVersionsOf(c.id)[0].date : '' }}
              </div>
            </div>
            <div class="row-side" style="gap: 8px; flex-wrap: wrap; justify-content: flex-end">
              <template v-if="centralVersionsOf(c.id)[0].assets.length">
                <button
                  v-for="a in centralVersionsOf(c.id)[0].assets"
                  :key="a.url"
                  class="btn-grad"
                  style="font-size: 12px; padding: 7px 15px"
                  @click="askDownload(c, centralVersionsOf(c.id)[0], a)"
                >
                  {{ shortAsset(a.name) }}{{ a.size ? ' · ' + fmtBytes(a.size) : '' }}
                </button>
              </template>
              <button
                v-else
                class="btn-ghost"
                style="font-size: 12px; padding: 6px 12px"
                @click="openPage(centralVersionsOf(c.id)[0].pageUrl)"
              >
                {{ centralVersionsOf(c.id)[0].folder ? '打开目录' : '页面' }}
              </button>
            </div>
          </div>

          <!-- 其余旧版本默认折叠 -->
          <template v-if="centralVersionsOf(c.id).length > 1">
            <button class="old-toggle" @click="showOld[c.id] = !showOld[c.id]">
              {{ showOld[c.id] ? '收起旧版本' : `展开旧版本（${centralVersionsOf(c.id).length - 1}）` }}
            </button>
            <div v-show="showOld[c.id]" class="old-list">
              <div
                v-for="(v, vi) in centralVersionsOf(c.id).slice(1)"
                :key="c.id + '-old-' + vi"
                class="row-card"
                style="padding: 10px 14px"
              >
                <div class="row-main">
                  <div class="name" style="font-size: 14px">
                    {{ v.tag }}
                    <span
                      v-if="v.dropped"
                      class="tag"
                      style="margin-left: 6px; background: rgba(255,255,255,.07); color: var(--ink-dim)"
                    >
                      已滤除 {{ v.dropped }} 个服务端
                    </span>
                  </div>
                  <div class="meta">{{ v.title }}{{ v.date ? ' · ' + v.date : '' }}</div>
                </div>
                <div class="row-side" style="gap: 8px; flex-wrap: wrap; justify-content: flex-end">
                  <template v-if="v.assets.length">
                    <button
                      v-for="a in v.assets"
                      :key="a.url"
                      class="btn-ghost"
                      style="color: var(--cyan); font-size: 12px; padding: 6px 12px"
                      @click="askDownload(c, v, a)"
                    >
                      {{ shortAsset(a.name) }}{{ a.size ? ' · ' + fmtBytes(a.size) : '' }}
                    </button>
                  </template>
                  <button v-else class="btn-ghost" style="font-size: 12px; padding: 6px 12px" @click="openPage(v.pageUrl)">
                    {{ v.folder ? '打开目录' : '页面' }}
                  </button>
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </template>

    <!-- 本地源表（预置源已下线，只有手动保留的才会显示） -->
    <template v-if="groups.length">
      <div v-for="g in groups" :key="g.name" style="margin-bottom: 16px">
        <button
          class="btn-ghost"
          style="display: flex; align-items: center; gap: 8px; margin-bottom: 10px"
          @click="toggleGroup(g.name)"
        >
          <span>{{ groupOpen[g.name] === false ? '▸' : '▾' }}</span>
          <span style="font-weight: 600">{{ g.name }}</span>
          <span style="color: var(--ink-dim)">{{ g.items.length }} 个源</span>
        </button>

        <div v-show="groupOpen[g.name] !== false" class="list">
          <div v-for="item in g.items" :key="item.id" class="row-card" style="flex-wrap: wrap">
            <div class="row-icon" :class="{ cy: item.kind === 'github-repo' }">
              {{ item.kind === 'github-repo' ? '🐙' : '⬇' }}
            </div>
            <div class="row-main" style="min-width: 240px">
              <div class="name">
                {{ item.name }}
                <span class="tag" style="margin-left: 6px">
                  {{ item.kind === 'github-repo' ? '仓库' : '直链' }}
                </span>
              </div>
              <div class="meta" style="word-break: break-all">
                {{ item.kind === 'github-repo' ? item.repo : item.url }}
              </div>
              <div v-if="item.note" class="meta">{{ item.note }}</div>
            </div>

            <div class="row-side" style="flex-wrap: wrap; gap: 8px">
              <!-- 仓库源：展开版本 -->
              <template v-if="item.kind === 'github-repo'">
                <button class="btn-grad" style="font-size: 13px; padding: 8px 20px" @click="fetchVersions(item)">
                  {{ expanded[item.id] ? '收起版本' : '展开版本' }}
                </button>
              </template>

              <!-- 直链源：下载状态机 -->
              <template v-else>
                <template v-if="stateOf(fileNameForDirect(item))?.status === 'downloading'">
                  <div class="dl-progress">
                    <div class="bar"><i :style="{ width: stateOf(fileNameForDirect(item))!.percent + '%' }" /></div>
                    <div class="pct">
                      {{ stateOf(fileNameForDirect(item))!.total > 0
                        ? stateOf(fileNameForDirect(item))!.percent.toFixed(0) + '%'
                        : fmtBytes(stateOf(fileNameForDirect(item))!.received) }}
                    </div>
                  </div>
                  <button class="btn-ghost" style="color: #ff8f8f" @click="cancel(fileNameForDirect(item), item.id)">取消</button>
                </template>
                <template v-else-if="stateOf(fileNameForDirect(item))?.status === 'done'">
                  <span class="tag" style="background: rgba(110,231,249,.15)">{{ baseName(stateOf(fileNameForDirect(item))!.path!) }}</span>
                  <button
                    class="btn-grad"
                    style="font-size: 13px; padding: 8px 18px"
                    :disabled="busy"
                    @click="createInstanceFrom(fileNameForDirect(item), item.name, item.id)"
                  >
                    创建游戏
                  </button>
                  <button class="btn-ghost" @click="startDirect(item)">重新下载</button>
                </template>
                <template v-else-if="stateOf(fileNameForDirect(item))?.status === 'error'">
                  <span class="tag" style="background: rgba(255,120,120,.15); color: #ff8f8f">
                    失败 code={{ stateOf(fileNameForDirect(item))!.code }}
                  </span>
                  <button class="btn-ghost" @click="startDirect(item)">重试</button>
                  <button class="btn-ghost" @click="mirrorRetry(item.url, fileNameForDirect(item), item.id)">镜像重试</button>
                </template>
                <template v-else>
                  <button class="btn-grad" style="font-size: 13px; padding: 8px 22px" @click="startDirect(item)">下载</button>
                </template>
              </template>

              <button
                v-if="item.openInNewPage"
                class="btn-ghost"
                @click="openPage(item.kind === 'github-repo' ? `https://github.com/${item.repo}/releases` : item.url)"
              >
                页面
              </button>
              <button class="btn-ghost" style="color: #ff8f8f" @click="removeSource(item)">删除</button>
            </div>

            <div v-if="cardErrors[item.id]" style="flex-basis: 100%; color: #ff7db0; font-size: 12px">
              {{ cardErrors[item.id] }}
            </div>

            <!-- 仓库源版本列表 -->
            <div
              v-if="item.kind === 'github-repo' && expanded[item.id]"
              style="flex-basis: 100%; margin-top: 6px"
            >
              <div v-if="versions[item.id]?.loading" class="meta" style="padding: 8px 0">加载版本列表…</div>
              <div v-else-if="versions[item.id]?.error" style="color: #ff8f8f; font-size: 12px; padding: 8px 0">
                {{ versions[item.id].error }}
              </div>
              <div v-else class="list" style="gap: 8px">
                <div v-if="(versions[item.id]?.items.length ?? 0) > 1" style="padding-bottom: 2px">
                  <button
                    class="btn-ghost"
                    style="font-size: 12px; padding: 5px 12px"
                    @click="showAllVersions[item.id] = !showAllVersions[item.id]"
                  >
                    {{
                      showAllVersions[item.id]
                        ? '收起其他版本'
                        : `展开全部版本（${versions[item.id]?.items.length ?? 0}）`
                    }}
                  </button>
                </div>
                <div
                  v-for="(v, idx) in shownVersions(item.id)"
                  :key="v.tag"
                  class="row-card"
                  style="padding: 10px 14px"
                >
                  <div class="row-main">
                    <div class="name" style="font-size: 14px">
                      {{ v.title || v.tag }}
                      <span class="meta" style="margin-left: 8px">{{ v.updated.slice(0, 10) }}</span>
                    </div>
                  </div>
                  <div class="row-side" style="gap: 8px">
                    <span
                      v-if="idx === 0 && item.latestEnabled"
                      class="tag"
                      style="background: rgba(255,125,176,.16); color: var(--pink)"
                    >
                      最新
                    </span>

                    <template v-if="stateOf(fileNameForVersion(item, v))?.status === 'downloading'">
                      <div class="dl-progress" style="width: 120px">
                        <div class="bar"><i :style="{ width: stateOf(fileNameForVersion(item, v))!.percent + '%' }" /></div>
                        <div class="pct">{{ stateOf(fileNameForVersion(item, v))!.percent.toFixed(0) }}%</div>
                      </div>
                      <button class="btn-ghost" @click="cancel(fileNameForVersion(item, v), item.id + ':' + v.tag)">取消</button>
                    </template>
                    <template v-else-if="stateOf(fileNameForVersion(item, v))?.status === 'done'">
                      <button
                        class="btn-grad"
                        style="font-size: 12px; padding: 6px 14px"
                        :disabled="busy"
                        @click="createInstanceFrom(fileNameForVersion(item, v), `${v.tag}`, item.id + ':' + v.tag)"
                      >
                        创建游戏
                      </button>
                    </template>
                    <template v-else-if="stateOf(fileNameForVersion(item, v))?.status === 'error'">
                      <span class="tag" style="background: rgba(255,120,120,.15); color: #ff8f8f">失败</span>
                      <button class="btn-ghost" @click="startVersion(item, v)">重试</button>
                      <button
                        class="btn-ghost"
                        @click="mirrorRetry(v.jarUrl, fileNameForVersion(item, v), item.id + ':' + v.tag)"
                      >
                        镜像重试
                      </button>
                    </template>
                    <template v-else>
                      <button class="btn-ghost" style="color: var(--cyan)" @click="startVersion(item, v)">下载</button>
                    </template>

                    <button
                      v-if="item.openInNewPage"
                      class="btn-ghost"
                      @click="openPage(v.pageUrl)"
                    >
                      页面
                    </button>
                  </div>
                  <div
                    v-if="cardErrors[item.id + ':' + v.tag]"
                    style="flex-basis: 100%; color: #ff7db0; font-size: 12px"
                  >
                    {{ cardErrors[item.id + ':' + v.tag] }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
    <div v-else class="empty">本地源表为空 · 下载走上方的中心索引</div>

    <!-- 下载创建面板：字段留空即使用默认值 -->
    <div v-if="dlPanel" class="edit-overlay" @click.self="closePanel">
      <div class="set-group" style="max-width: 680px; margin: 40px auto">
        <h3>下载并创建 · {{ dlPanel.sourceName }} {{ dlPanel.version }}</h3>

        <div class="set-row">
          <div class="label">游戏名称</div>
          <div class="ctrl" style="flex: 1; max-width: 340px">
            <input v-model="dlPanel.name" class="field" style="width: 100%" :disabled="dlPanel.state !== 'form'" />
          </div>
        </div>

        <div class="set-row">
          <div class="label">下载目录</div>
          <div class="ctrl" style="flex: 1; max-width: 340px">
            <input
              v-model="dlPanel.downloadDir"
              class="field"
              style="width: 100%"
              :disabled="dlPanel.state !== 'form'"
              placeholder="留空 = 默认下载目录"
            />
          </div>
        </div>

        <div class="set-row">
          <div class="label">内存 (MB)</div>
          <div class="ctrl">
            <input
              v-model.number="dlPanel.memoryMb"
              type="range"
              min="1024"
              max="16384"
              step="512"
              :disabled="dlPanel.state !== 'form'"
            />
            <span class="val">{{ dlPanel.memoryMb }} MB</span>
          </div>
        </div>

        <div class="set-row">
          <div class="label">数据隔离</div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="dlPanel.isolate" type="checkbox" :disabled="dlPanel.state !== 'form'" />
              <span class="track" />
              <span class="thumb" />
            </label>
          </div>
        </div>

        <div class="set-row">
          <div class="label">Java 运行时</div>
          <div class="ctrl" style="flex: 1; max-width: 340px">
            <input
              v-model="dlPanel.javaPath"
              class="field"
              style="width: 100%"
              :disabled="dlPanel.state !== 'form'"
              placeholder="留空 = java"
            />
          </div>
        </div>

        <div class="set-row">
          <div class="label">JVM 参数</div>
          <div class="ctrl" style="flex: 1; max-width: 340px">
            <input
              v-model="dlPanel.jvmArgs"
              class="field"
              style="width: 100%"
              :disabled="dlPanel.state !== 'form'"
              placeholder="留空 = 不加"
            />
          </div>
        </div>

        <div class="set-row">
          <div class="hint">
            全部留空即用默认值：下载目录 = 设置里的下载目录 · 内存 = {{ settings.memory }} MB ·
            隔离 = {{ settings.saveIsolation ? '开' : '关' }} · Java = java · JVM 参数不加
          </div>
        </div>

        <p v-if="dlPanel.error" style="color: #ff7db0; font-size: 13px; padding: 8px 0">{{ dlPanel.error }}</p>
        <p v-if="dlPanel.state === 'done'" style="color: var(--cyan); font-size: 13px; padding: 8px 0">
          已创建游戏「{{ dlPanel.name }}」
        </p>

        <div class="set-row" style="justify-content: flex-end; gap: 10px">
          <template v-if="dlPanel.state === 'form'">
            <button class="btn-ghost" @click="closePanel">取消</button>
            <button class="btn-grad" style="font-size: 14px; padding: 9px 26px" @click="confirmDownload">
              下载并创建
            </button>
          </template>
          <template v-else-if="dlPanel.state === 'downloading'">
            <span class="meta">下载中 {{ downloadStates[dlPanel.fileName]?.percent?.toFixed(0) ?? 0 }}%</span>
          </template>
          <template v-else-if="dlPanel.state === 'creating'">
            <span class="meta">正在创建游戏…</span>
          </template>
          <template v-else-if="dlPanel.state === 'done'">
            <button class="btn-grad" style="font-size: 14px; padding: 9px 26px" @click="closePanel">完成</button>
          </template>
          <template v-else>
            <button class="btn-ghost" @click="closePanel">关闭</button>
            <button class="btn-grad" style="font-size: 14px; padding: 9px 26px" @click="confirmDownload">重试</button>
          </template>
        </div>
      </div>
    </div>
  </section>
</template>
