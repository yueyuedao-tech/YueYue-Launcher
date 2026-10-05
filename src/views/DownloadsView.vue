<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import {
  downloadStates,
  instances,
  loadInstances,
  settings,
  store,
  type DlState,
} from '../store'
import type { CentralIndex, CentralItem, GithubVersion, SourceItem } from '../types'

/** 源表行：本地源 + 中心索引行（中心行不可编辑/删除） */
type RowItem = SourceItem & { fromCentral?: boolean; tags?: string[]; size?: number }

const sources = ref<SourceItem[]>([])
const central = ref<CentralItem[]>([])
const centralMeta = ref({ source: '', note: '' })
const centralLoading = ref(false)
const subsError = ref('')
const cardErrors = reactive<Record<string, string>>({})
const busy = ref(false)

// 分组折叠状态（初始取组内任一源的 collapsed）
const groupOpen = reactive<Record<string, boolean>>({})
// github 源展开状态与版本缓存
const expanded = reactive<Record<string, boolean>>({})
const versions = reactive<
  Record<string, { loading: boolean; error: string; items: GithubVersion[] }>
>({})
/** 仓库源版本列表：最新一条常显，其余版本默认收起 */
const showAllVersions = reactive<Record<string, boolean>>({})

function shownVersions(id: string): GithubVersion[] {
  const all = versions[id]?.items ?? []
  return showAllVersions[id] ? all : all.slice(0, 1)
}

const showAdd = ref(false)
const form = reactive({
  kind: 'direct-url' as 'direct-url' | 'github-repo',
  name: '',
  url: '',
  repo: '',
  asset: 'Mindustry.jar',
  note: '',
  group: '默认',
  collapsed: false,
  latestEnabled: true,
  openInNewPage: false,
})

/** 中心索引行 → 源表行形态，复用同一套下载/版本/进度逻辑 */
function toRow(c: CentralItem): RowItem {
  return {
    id: `c-${c.id}`,
    name: c.name,
    kind: c.kind,
    url: c.url,
    repo: c.repo,
    asset: c.asset,
    note: c.note,
    group: `中心 · ${c.group}`,
    collapsed: false,
    latestEnabled: true,
    openInNewPage: false,
    fromCentral: true,
    tags: c.tags,
    size: c.size,
  }
}

const groups = computed(() => {
  const order: string[] = []
  const map: Record<string, RowItem[]> = {}
  const push = (s: RowItem) => {
    if (!map[s.group]) {
      map[s.group] = []
      order.push(s.group)
    }
    map[s.group].push(s)
  }
  central.value.forEach((c) => push(toRow(c)))
  sources.value.forEach((s) => push(s))
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
  let base = raw.replace(/[\\/:*?"<>|]/g, '_').trim().slice(0, 36) || '实例'
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

async function refreshAll() {
  await Promise.all([loadCentral(), loadSources()])
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

async function addSource() {
  busy.value = true
  try {
    const item: SourceItem = {
      id: 's' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6),
      name: form.name.trim(),
      kind: form.kind,
      url: form.kind === 'direct-url' ? form.url.trim() : '',
      repo: form.kind === 'github-repo' ? form.repo.trim() : '',
      asset: form.asset.trim() || 'Mindustry.jar',
      note: form.note.trim(),
      group: form.group.trim() || '默认',
      collapsed: form.collapsed,
      latestEnabled: form.latestEnabled,
      openInNewPage: form.openInNewPage,
    }
    sources.value = [...sources.value, item]
    await persist()
    if (!subsError.value) {
      form.name = ''
      form.url = ''
      form.repo = ''
      form.note = ''
      showAdd.value = false
      await loadSources()
    }
  } finally {
    busy.value = false
  }
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

onMounted(() => {
  void loadCentral()
  void loadSources()
})
</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">下载中心</h1>
      </div>
      <div style="display: flex; gap: 10px">
        <button class="btn-ghost" :disabled="busy" @click="showAdd = !showAdd">
          {{ showAdd ? '收起' : '+ 添加源' }}
        </button>
        <button class="btn-ghost" :disabled="busy || centralLoading" @click="refreshAll">刷新列表</button>
      </div>
    </div>

    <p v-if="subsError" style="color: #ff7db0; font-size: 13px; margin-bottom: 12px">{{ subsError }}</p>
    <p v-if="centralMeta.note" style="color: var(--amber); font-size: 12px; margin-bottom: 12px">
      {{ centralMeta.note }}
    </p>

    <!-- 添加源 -->
    <div v-if="showAdd" class="set-group" style="margin-bottom: 18px">
      <h3>添加下载源</h3>
      <div class="set-row">
        <div class="label">类型</div>
        <div class="ctrl">
          <select v-model="form.kind" class="field">
            <option value="direct-url">直链 URL</option>
            <option value="github-repo">GitHub 仓库</option>
          </select>
        </div>
      </div>
      <div class="set-row">
        <div class="label">名称</div>
        <div class="ctrl" style="flex: 1; max-width: 520px">
          <input v-model="form.name" class="field" style="width: 100%" placeholder="如：Mindustry 官方仓库" />
        </div>
      </div>
      <div v-if="form.kind === 'direct-url'" class="set-row">
        <div class="label">URL<div class="hint">必须以 http(s):// 开头</div></div>
        <div class="ctrl" style="flex: 1; max-width: 640px">
          <input v-model="form.url" class="field" style="width: 100%" placeholder="https://…/Mindustry.jar" />
        </div>
      </div>
      <template v-else>
        <div class="set-row">
          <div class="label">仓库<div class="hint">owner/name，如 Anuken/Mindustry</div></div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="form.repo" class="field" style="width: 100%" placeholder="Anuken/Mindustry" />
          </div>
        </div>
        <div class="set-row">
          <div class="label">下载资产名<div class="hint">每个 release 里要下的文件</div></div>
          <div class="ctrl" style="flex: 1; max-width: 320px">
            <input v-model="form.asset" class="field" style="width: 100%" />
          </div>
        </div>
      </template>
      <div class="set-row">
        <div class="label">分组</div>
        <div class="ctrl">
          <input v-model="form.group" class="field" placeholder="默认" style="width: 160px" />
        </div>
      </div>
      <div class="set-row">
        <div class="label">备注</div>
        <div class="ctrl" style="flex: 1; max-width: 520px">
          <input v-model="form.note" class="field" style="width: 100%" placeholder="可选" />
        </div>
      </div>
      <div class="set-row">
        <div class="label">初始折叠</div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="form.collapsed" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <div class="set-row">
        <div class="label">显示「最新」徽标<div class="hint">仓库源第一条 release 标记最新</div></div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="form.latestEnabled" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <div class="set-row">
        <div class="label">提供「页面」按钮<div class="hint">用系统浏览器打开 release/资源页</div></div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="form.openInNewPage" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <div class="set-row" style="justify-content: flex-end; gap: 10px">
        <button class="btn-grad" style="font-size: 14px; padding: 9px 26px" :disabled="busy" @click="addSource">
          添加
        </button>
      </div>
    </div>

    <!-- 分组源列表 -->
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
                <span
                  v-for="t in item.tags ?? []"
                  :key="t"
                  class="tag"
                  style="margin-left: 6px; background: rgba(110, 231, 249, 0.14)"
                >
                  {{ t }}
                </span>
                <span v-if="!item.tags?.length" class="tag" style="margin-left: 6px">
                  {{ item.kind === 'github-repo' ? '仓库' : '直链' }}
                </span>
              </div>
              <div class="meta" style="word-break: break-all">
                {{ item.kind === 'github-repo' ? item.repo : item.url }}
              </div>
              <div v-if="item.note || item.size" class="meta">
                <template v-if="item.note">{{ item.note }}</template>
                <template v-if="item.note && item.size"> · </template>
                <template v-if="item.size">{{ fmtBytes(item.size) }}</template>
              </div>
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
                    创建实例
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
              <button v-if="!item.fromCentral" class="btn-ghost" style="color: #ff8f8f" @click="removeSource(item)">删除</button>
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
                        创建实例
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
    <div v-else class="empty">源表为空——点右上角「添加源」</div>
  </section>
</template>
