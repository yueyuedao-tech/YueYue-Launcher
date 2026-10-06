<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import {
  centralItems,
  centralLoading,
  centralMeta,
  centralVersions,
  downloadStates,
  downloadTasks,
  effectiveProxy,
  instances,
  loadCentralIndex,
  loadInstances,
  markDownloadCanceled,
  refreshDownloadTasks,
  settings,
  store,
  syncCentralVersions,
  taskPanelOpen,
  unmarkDownloadCanceled,
  versionSyncing,
  type DlState,
} from '../store'
import type {
  CentralAsset,
  CentralItem,
  CentralVersion,
  DownloadDonePayload,
  DownloadErrorPayload,
  GithubVersion,
  SourceItem,
} from '../types'

const sources = ref<SourceItem[]>([])
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

/** logo 拉取失败时回退到首字母，避免图标空着 */
const logoFailed = reactive<Record<string, boolean>>({})
function logoOf(c: CentralItem): string {
  return logoFailed[c.id] ? '' : c.logo
}

/* ---------- 下载中心内搜索：源名 / 标签 / 版本号 ---------- */
const query = ref('')
const q = computed(() => query.value.trim().toLowerCase())

function contains(text: string): boolean {
  return !q.value || String(text ?? '').toLowerCase().includes(q.value)
}

function sourceHit(c: CentralItem): boolean {
  return (
    contains(c.name) ||
    contains(c.url) ||
    contains(c.note) ||
    contains(c.slogan) ||
    c.tags.some((t) => contains(t))
  )
}

function versionHit(v: CentralVersion): boolean {
  return contains(v.tag) || contains(v.title) || v.assets.some((a) => contains(a.name))
}

/** 命中的源（无搜索时全部显示） */
const visibleCentral = computed(() =>
  centralItems.value.filter((c) => sourceHit(c) || centralVersionsOf(c.id).some(versionHit)),
)

/** 当前源下要展示的版本：源本身命中→全部；否则只留命中的 */
function versionsFor(c: CentralItem): CentralVersion[] {
  const all = centralVersionsOf(c.id)
  if (!q.value) return all
  if (sourceHit(c)) return all
  return all.filter(versionHit)
}

/** 旧版本按主版本号归档：最新版单独置顶，不纳入归档 */
function majorOf(tag: string): string {
  const m = tag.match(/\d+/)
  return m ? m[0] : '其他'
}

interface VerGroup {
  key: string
  label: string
  items: CentralVersion[]
}

function oldGroups(c: CentralItem): VerGroup[] {
  const rest = versionsFor(c).slice(1)
  const order: string[] = []
  const map: Record<string, CentralVersion[]> = {}
  for (const v of rest) {
    const k = majorOf(v.tag)
    if (!map[k]) {
      map[k] = []
      order.push(k)
    }
    map[k].push(v)
  }
  return order.map((k) => ({
    key: k,
    label: /^\d+$/.test(k) ? `v${k} 系列` : '其他版本',
    items: map[k],
  }))
}

/** 归档区（vXXX 系列分组）平时折在最新版那一栏下面；点最新版整行展开/收起 */
const archiveOpen = reactive<Record<string, boolean>>({})

function isArchiveOpen(c: CentralItem): boolean {
  return !!q.value || !!archiveOpen[c.id]
}

function toggleArchive(c: CentralItem) {
  archiveOpen[c.id] = !archiveOpen[c.id]
}

/** 归档分组默认收起；搜索时强制展开，免得命中的版本藏在里面 */
const verGroupOpen = reactive<Record<string, boolean>>({})

function isGroupOpen(c: CentralItem, key: string): boolean {
  return !!q.value || !!verGroupOpen[`${c.id}:${key}`]
}

function toggleVerGroup(c: CentralItem, key: string) {
  const k = `${c.id}:${key}`
  verGroupOpen[k] = !verGroupOpen[k]
}

/** 展示序列里的第一条（最新版，独立置顶） */
function latestOf(c: CentralItem): CentralVersion | undefined {
  return versionsFor(c)[0]
}

/** 置顶卡片是不是「真·最新版」：搜索命中旧版本时不能冒充最新 */
function isTrueLatest(c: CentralItem): boolean {
  const all = centralVersionsOf(c.id)
  const shown = versionsFor(c)[0]
  return !!all.length && !!shown && all[0].tag === shown.tag
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

/** 本地源也吃同一个搜索词：名称 / 仓库 / 直链 / 备注 */
function localHit(s: SourceItem): boolean {
  return contains(s.name) || contains(s.repo) || contains(s.url) || contains(s.note)
}

/** 搜索时只留命中的本地源；整组没命中就不显示组头 */
const visibleGroups = computed(() =>
  groups.value
    .map((g) => ({ name: g.name, items: g.items.filter((s) => !q.value || localHit(s)) }))
    .filter((g) => g.items.length > 0),
)

/** 中心 + 本地两边都没有命中才算「搜不到」 */
const nothingMatched = computed(() => !visibleCentral.value.length && !visibleGroups.value.length)

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

/** 查询中心化服务器索引；失败由后端降级为内置索引，这里只负责提示。
 *  索引在开机时（App.vue）已经拉过一次，这里通常直接命中，不再等网络。 */
async function loadCentral() {
  await loadCentralIndex()
  prefetchFolders()
}

// 版本索引可能晚于挂载才到（开机同步是后台跑的），到了就补预取，
// 否则文件站目录永远是冷的，点下载就得现等好几秒
watch([centralItems, centralVersions], () => prefetchFolders(), { immediate: true })

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
      proxy: effectiveProxy.value,
      prefix: settings.githubPrefix,
    })) as GithubVersion[]
    versions[s.id] = { loading: false, error: '', items }
  } catch (e) {
    versions[s.id] = { loading: false, error: String(e), items: [] }
  }
}

async function startUrl(
  url: string,
  fileName: string,
  mirrorRetry: boolean,
  errKey: string,
  threads: number = settings.downloadThreads,
  name: string = fileName,
) {
  cardErrors[errKey] = ''
  // 这条要重新开始了：解掉「已取消」标记，进度事件才能再进来
  unmarkDownloadCanceled(fileName)
  downloadStates[fileName] = {
    status: 'downloading',
    percent: 0,
    received: 0,
    total: 0,
    threads,
  }
  try {
    await invoke('start_download', {
      url: applyPrefix(url, mirrorRetry),
      fileName,
      proxy: effectiveProxy.value,
      downloadDir: settings.downloadDir,
      threads,
      name,
    })
    // 立刻同步一次后端任务表：角标和抽屉马上就有这条（名字/URL 也才拿得到）
    void refreshDownloadTasks()
  } catch (e) {
    const msg = String(e)
    // 后端回「已在下载中」说明这条本来就在跑：以前端状态为准会把它误标成失败，
    // 这里改用后端任务表对齐（速度/线程/进度都还在推送）
    if (msg.includes('已在下载中')) {
      await refreshDownloadTasks()
      const t = downloadTasks.value.find((x) => x.fileName === fileName)
      downloadStates[fileName] = {
        status: t?.status === 'error' ? 'error' : 'downloading',
        percent: t?.percent ?? 0,
        received: t?.received ?? 0,
        total: t?.total ?? 0,
        speed: t?.speed,
        threads: t?.threads,
        code: t?.status === 'error' ? t.code : undefined,
      }
      cardErrors[errKey] = ''
      return
    }
    downloadStates[fileName].status = 'error'
    cardErrors[errKey] = msg
  }
}

function startDirect(item: SourceItem) {
  void startUrl(item.url, fileNameForDirect(item), false, item.id, settings.downloadThreads, item.name)
}

function startVersion(s: SourceItem, v: GithubVersion) {
  void startUrl(
    v.jarUrl,
    fileNameForVersion(s, v),
    false,
    s.id + ':' + v.tag,
    settings.downloadThreads,
    `${s.name} ${v.tag}`,
  )
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
  markDownloadCanceled(fileName)
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

/** 文件站目录缓存：同一个 源+版本 只拉一次（含在途请求） */
const folderFiles = reactive<
  Record<string, { loading: boolean; error: string; items: CentralAsset[] }>
>({})
const folderPending: Record<string, Promise<CentralAsset[]> | undefined> = {}
/** 正在为哪一条「下载」读取目录（按钮转圈用） */
const mirrorPicking = reactive<Record<string, boolean>>({})

function folderKey(c: CentralItem, v: CentralVersion): string {
  return `${c.id}:${v.tag}`
}

/** 取某个文件站目录的文件清单：命中缓存就直接返回，在途请求复用同一个 Promise */
function loadFolder(c: CentralItem, v: CentralVersion): Promise<CentralAsset[]> {
  const k = folderKey(c, v)
  const cached = folderFiles[k]
  if (cached?.items.length) return Promise.resolve(cached.items)
  if (folderPending[k]) return folderPending[k]
  folderFiles[k] = { loading: true, error: '', items: [] }
  const p = (async () => {
    try {
      const items = (await invoke('list_folder_files', {
        url: v.pageUrl,
        proxy: effectiveProxy.value,
      })) as CentralAsset[]
      folderFiles[k] = { loading: false, error: '', items }
      return items
    } catch (e) {
      folderFiles[k] = { loading: false, error: String(e), items: [] }
      throw e
    } finally {
      delete folderPending[k]
    }
  })()
  folderPending[k] = p
  return p
}

/** 后台预取文件站最新版的目录内容：点「下载」时就能立刻弹面板，不用干等网络 */
function prefetchFolders() {
  for (const c of centralItems.value) {
    if (c.kind !== 'file-list') continue
    const v = centralVersionsOf(c.id)[0]
    if (!v?.folder) continue
    const k = folderKey(c, v)
    if (folderFiles[k]?.items.length || folderPending[k]) continue
    loadFolder(c, v).catch(() => {
      /* 预取失败不打扰用户，点下载时会再试一次 */
    })
  }
}

/** 自动挑客户端 jar：优先 Mindustry.jar，其次第一个 jar（服务端包已由后端过滤掉） */
function pickClientJar(items: CentralAsset[]): CentralAsset | undefined {
  const jars = items.filter((a) => a.name.toLowerCase().endsWith('.jar'))
  return jars.find((a) => /^mindustry\.jar$/i.test(a.name)) ?? jars[0]
}

/** 文件站的「下载」：目录已预取时立即弹面板，没有才现拉 */
async function downloadFromFolder(c: CentralItem, v: CentralVersion) {
  const k = folderKey(c, v)
  if (mirrorPicking[k]) return
  mirrorPicking[k] = true
  cardErrors[c.id] = ''
  try {
    const items = await loadFolder(c, v)
    const pick = pickClientJar(items)
    if (!pick) {
      cardErrors[c.id] = '这个目录里没有可下载的客户端 jar'
      return
    }
    askDownload(c, v, pick)
  } catch (e) {
    cardErrors[c.id] = String(e)
  } finally {
    mirrorPicking[k] = false
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
  // 取整：否则很小的值会打成 0.00000000 这种
  return Math.round(n) + ' B'
}

function fmtSpeed(bps?: number): string {
  return bps && bps > 0 ? fmtBytes(bps) + '/s' : '测速中…'
}

function baseName(p: string) {
  return p.split(/[\\/]/).pop() ?? p
}

/** 旧版本按主版本号归档（见 oldGroups），不再有单个「展开旧版本」按钮 */

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
  threads: number
  state: 'form' | 'downloading' | 'creating' | 'done' | 'error'
  error: string
}

/** 点「下载」直接弹出创建面板；所有字段留空即走默认值 */
const dlPanel = ref<DlState2 | null>(null)
/** 面板是否可见。与 dlPanel 分开：关掉面板 ≠ 放弃「下载并创建」的意图 */
const dlPanelOpen = ref(false)
/** 待创建的游戏，按下载文件名存：关面板、或又被别的下载顶掉，都不会丢 */
const pendingCreates = reactive<Record<string, DlState2>>({})

function defaultFileName(sourceName: string, version: string, url: string): string {
  const ext = url.match(/\.[A-Za-z0-9]{1,7}$/)?.[0] ?? '.jar'
  const raw = `${sourceName}-${version}`.replace(/[\\/:*?"<>|]+/g, '_').trim()
  return (raw || 'download').slice(0, 80) + ext
}

function askDownload(c: CentralItem, v: CentralVersion, a: CentralAsset) {
  const p: DlState2 = {
    sourceName: c.name,
    version: v.tag,
    url: a.url,
    fileName: defaultFileName(c.name, v.tag, a.url),
    name: `${c.name} ${v.tag}`.slice(0, 40),
    downloadDir: settings.downloadDir,
    memoryMb: settings.memory,
    isolate: settings.saveIsolation,
    javaPath: 'java',
    jvmArgs: settings.defaultJvmArgs,
    threads: settings.downloadThreads,
    state: 'form',
    error: '',
  }
  pendingCreates[p.fileName] = p
  dlPanel.value = p
  dlPanelOpen.value = true
}

/** 关掉面板：下载/创建过程不受影响，任务在右下角圆圈里继续。
 *  以前这里在 downloading/creating 时直接 return，导致「下载中点 ✕ 没反应」。 */
function closePanel() {
  const p = dlPanel.value
  dlPanelOpen.value = false
  // 还没点「下载并创建」就关掉：这条意图作废，别留在待创建表里
  if (p && p.state === 'form') delete pendingCreates[p.fileName]
}

/** 「查看下载任务」：先收起面板再开抽屉。
 *  弹窗层级(95) 现在高于抽屉(80)，不收起面板的话抽屉是开在它后面的，等于点不动。 */
function viewTasks() {
  dlPanelOpen.value = false
  taskPanelOpen.value = true
}

/** ESC 关闭：先关下载弹窗，再关右下角任务抽屉 */
function onKeydown(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (dlPanelOpen.value) {
    closePanel()
    return
  }
  if (taskPanelOpen.value) taskPanelOpen.value = false
}

async function confirmDownload() {
  const p = dlPanel.value
  if (!p) return
  p.state = 'downloading'
  p.error = ''
  unmarkDownloadCanceled(p.fileName)
  // 必须自己先建一条进度记录：App 层的 progress 监听只在已有记录时才更新，
  // 否则弹窗里的百分比/速度会一直是 0% 与「测速中…」
  downloadStates[p.fileName] = {
    status: 'downloading',
    percent: 0,
    received: 0,
    total: 0,
    threads: p.threads,
  }
  try {
    await invoke('start_download', {
      url: p.url,
      fileName: p.fileName,
      proxy: effectiveProxy.value,
      downloadDir: p.downloadDir,
      threads: p.threads,
      name: p.name,
    })
    void refreshDownloadTasks()
    // 点完「下载并创建」直接回下载页面：进度与创建结果都交给右下角任务圆圈，
    // 不再让面板挡在下载页上面等下载跑完
    dlPanelOpen.value = false
  } catch (e) {
    p.state = 'error'
    p.error = String(e)
  }
}

/** 下载完成后自动用面板里选的隔离/内存/路径创建游戏。
 *  按文件名查待创建表：面板关掉、或用户又去下了别的，这条意图依然有效。 */
async function createFromPanel(fileName: string, path: string) {
  const p = pendingCreates[fileName]
  if (!p) return
  const name = p.name.trim()
  // 名字重复就不建（面板里也会红字提示，这里兜住「下载期间名字被别人占了」）
  if (instances.value.some((i) => i.name === name)) {
    p.state = 'error'
    p.error = `游戏名「${name}」已存在，请换一个名字`
    return
  }
  p.state = 'creating'
  p.error = ''
  try {
    await invoke('create_instance', {
      name: p.name.trim(),
      jarPath: path,
      javaPath: p.javaPath.trim() || 'java',
      jvmArgs: p.jvmArgs.split(/\s+/).filter(Boolean),
      memoryMb: p.memoryMb,
      isolate: p.isolate,
    })
    p.state = 'done'
    await loadInstances()
    // 下载 + 创建都完成了 → 面板若还开着就自动收起，任务归到右下角圆圈里
    setTimeout(() => {
      if (dlPanel.value === p) {
        dlPanelOpen.value = false
        dlPanel.value = null
      }
      delete pendingCreates[fileName]
    }, 1600)
  } catch (e) {
    p.state = 'error'
    p.error = String(e)
  }
}

const unlisteners: UnlistenFn[] = []

/** 游戏名不能重复：重名时红字提示并禁止创建 */
const dlNameTaken = computed(() => {
  const p = dlPanel.value
  if (!p) return false
  const n = p.name.trim()
  return !!n && instances.value.some((i) => i.name === n)
})

onMounted(async () => {
  void loadCentral()
  void loadSources()
  window.addEventListener('keydown', onKeydown)
  unlisteners.push(
    await listen<DownloadDonePayload>('download-done', (e) => {
      void createFromPanel(e.payload.fileName, e.payload.path)
    }),
    await listen<DownloadErrorPayload>('download-error', (e) => {
      const p = pendingCreates[e.payload.fileName]
      if (p) {
        p.state = 'error'
        p.error = `下载失败 code=${e.payload.code}`
      }
    }),
  )
})

onBeforeUnmount(() => {
  unlisteners.forEach((u) => u())
  window.removeEventListener('keydown', onKeydown)
})

</script>

<template>
  <section class="page">
    <!-- 下载中心内搜索：源名 / 标语 / 标签 / 版本号（本地源表一起吃这个搜索词） -->
    <div class="search-bar">
      <input
        v-model="query"
        class="field"
        type="search"
        placeholder="搜索源、标语、标签或版本号"
        aria-label="下载中心搜索"
      />
      <button v-if="query" class="btn-ghost" @click="query = ''">清空</button>
      <span v-if="q" class="meta">
        命中 {{ visibleCentral.length }} 个中心源 ·
        {{ visibleGroups.reduce((n, g) => n + g.items.length, 0) }} 个本地源
      </span>
    </div>

    <p v-if="subsError" style="color: #ff7db0; font-size: 13px; margin-bottom: 12px">{{ subsError }}</p>
    <p v-if="centralMeta.note" style="color: var(--amber); font-size: 12px; margin-bottom: 12px">
      {{ centralMeta.note }}
    </p>

    <!-- 中心索引：名称 / 标签 / 版本均由中心服务器下发 -->
    <div v-if="centralLoading" class="empty">正在拉取中心索引…</div>
    <template v-else>
      <div v-if="nothingMatched" class="empty">没有匹配「{{ query }}」的源或版本</div>
      <div v-for="c in visibleCentral" :key="c.id" class="row-card" style="flex-direction: column; align-items: stretch; gap: 0; margin-bottom: 14px">
        <div style="display: flex; gap: 14px; align-items: flex-start">
          <div class="row-icon cy" style="overflow: hidden">
            <img v-if="logoOf(c)" :src="logoOf(c)" :alt="c.name" style="width:100%;height:100%;object-fit:cover" @error="logoFailed[c.id] = true" />
            <template v-else>{{ c.name.slice(0, 1) }}</template>
          </div>
          <div class="row-main">
            <div class="name">
              {{ c.name }}
              <span v-for="t in c.tags" :key="t" class="tag" style="margin-left: 6px">{{ t }}</span>
            </div>
          </div>
          <!-- 检查更新按钮已按用户要求移除 -->
        </div>

        <div class="vlist">
          <div v-if="versionSyncing && !centralVersionsOf(c.id).length" class="meta" style="padding: 8px 0">
            正在扫描版本…
          </div>
          <div v-else-if="!centralVersionsOf(c.id).length" class="meta" style="padding: 8px 0; color: #ff8f8f">
            {{ centralErrorOf(c.id) || '本地暂无版本缓存，索引完成后自动填充' }}
          </div>
          <div v-else-if="!versionsFor(c).length" class="meta" style="padding: 8px 0">
            该源下没有匹配「{{ query }}」的版本
          </div>

          <!-- 最新版独立置顶：整行就是旧版本归档的开合开关（里面的按钮不触发） -->
          <div
            v-if="latestOf(c)"
            :class="[
              isTrueLatest(c) ? 'row-card row-card--latest' : 'row-card',
              oldGroups(c).length ? 'row-card--toggle' : '',
            ]"
            style="padding: 13px 15px"
            @click="oldGroups(c).length && toggleArchive(c)"
          >
            <div class="row-main">
              <div class="name" style="font-size: 14.5px">
                <span v-if="oldGroups(c).length" class="archive-arrow">
                  {{ isArchiveOpen(c) ? '▾' : '▸' }}
                </span>
                {{ latestOf(c)!.tag }}
                <span v-if="isTrueLatest(c)" class="tag" style="margin-left: 6px; background: rgba(255,125,176,.16); color: var(--pink)">最新</span>
                <span v-else-if="q" class="tag" style="margin-left: 6px; background: rgba(255,255,255,.07); color: var(--ink-dim)">搜索命中</span>
                <span v-if="oldGroups(c).length" class="meta" style="margin-left: 8px; font-size: 12px">
                  旧版本 {{ versionsFor(c).length - 1 }} 个
                </span>
              </div>
              <div class="meta">
                {{ latestOf(c)!.title }}{{ latestOf(c)!.date ? ' · ' + latestOf(c)!.date : '' }}
              </div>
            </div>
            <div class="row-side" style="gap: 8px; flex-wrap: wrap; justify-content: flex-end">
              <template v-if="latestOf(c)!.folder">
                <!-- 文件站：不给「展开内容」，直接一个下载按钮，自动挑客户端 jar -->
                <button
                  class="btn-grad"
                  style="font-size: 12px; padding: 7px 15px"
                  :disabled="mirrorPicking[folderKey(c, latestOf(c)!)]"
                  @click.stop="downloadFromFolder(c, latestOf(c)!)"
                >
                  {{ mirrorPicking[folderKey(c, latestOf(c)!)] ? '读取中…' : '下载' }}
                </button>
                <button
                  class="btn-ghost"
                  style="font-size: 12px; padding: 6px 12px"
                  @click.stop="openPage(latestOf(c)!.pageUrl)"
                >
                  打开目录
                </button>
              </template>
              <template v-else-if="latestOf(c)!.assets.length">
                <button
                  v-for="a in latestOf(c)!.assets"
                  :key="a.url"
                  class="btn-grad"
                  style="font-size: 12px; padding: 7px 15px"
                  @click.stop="askDownload(c, latestOf(c)!, a)"
                >
                  {{ shortAsset(a.name) }}{{ a.size ? ' · ' + fmtBytes(a.size) : '' }}
                </button>
              </template>
              <button
                v-else
                class="btn-ghost"
                style="font-size: 12px; padding: 6px 12px"
                @click.stop="openPage(latestOf(c)!.pageUrl)"
              >
                页面
              </button>
            </div>
          </div>

          <!-- 旧版本归档：平时折在最新版那一栏下面，点最新版展开；
               展开后每个主版本分组还能各自用悬停方向键收起 -->
          <template v-if="isArchiveOpen(c)">
            <div v-for="g in oldGroups(c)" :key="'vg-' + c.id + '-' + g.key" class="ver-group">
            <button class="ver-group-head" @click="toggleVerGroup(c, g.key)">
              <span class="vg-arrow">{{ isGroupOpen(c, g.key) ? '▾' : '▸' }}</span>
              <span class="vg-title">{{ g.label }}</span>
              <span class="vg-count">{{ g.items.length }} 个版本</span>
            </button>
            <div v-show="isGroupOpen(c, g.key)" class="old-list">
              <div
                v-for="(v, vi) in g.items"
                :key="c.id + '-old-' + g.key + '-' + vi"
                class="row-card"
                style="padding: 10px 14px"
              >
                <div class="row-main">
                  <div class="name" style="font-size: 14px">{{ v.tag }}</div>
                  <div class="meta">{{ v.title }}{{ v.date ? ' · ' + v.date : '' }}</div>
                </div>
                <div class="row-side" style="gap: 8px; flex-wrap: wrap; justify-content: flex-end">
                  <template v-if="v.folder">
                    <button
                      class="btn-ghost"
                      style="color: var(--cyan); font-size: 12px; padding: 6px 12px"
                      :disabled="mirrorPicking[folderKey(c, v)]"
                      @click="downloadFromFolder(c, v)"
                    >
                      {{ mirrorPicking[folderKey(c, v)] ? '读取中…' : '下载' }}
                    </button>
                    <button class="btn-ghost" style="font-size: 12px; padding: 6px 12px" @click="openPage(v.pageUrl)">
                      打开目录
                    </button>
                  </template>
                  <template v-else-if="v.assets.length">
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
                    页面
                  </button>
                </div>
              </div>
            </div>
          </div>
          </template>
        </div>
      </div>
    </template>

    <!-- 本地源表（预置源已下线，只有手动保留的才会显示；搜索时一起过滤） -->
    <template v-if="visibleGroups.length">
      <div v-for="g in visibleGroups" :key="g.name" style="margin-bottom: 16px">
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
    <div v-else-if="!q" class="empty">本地源表为空 · 下载走上方的中心索引</div>

    <!-- 下载创建面板：字段留空即使用默认值。z-index 高于任务抽屉，ESC / 右上角 ✕ 都能关 -->
    <div v-if="dlPanel && dlPanelOpen" class="edit-overlay dl-overlay" @click.self="closePanel">
      <div class="set-group" style="max-width: 680px; margin: 40px auto">
        <div class="dl-head">
          <h3>下载并创建 · {{ dlPanel.sourceName }} {{ dlPanel.version }}</h3>
          <button class="dl-close" aria-label="关闭" title="关闭 (Esc)" @click="closePanel">✕</button>
        </div>

        <div class="set-row">
          <div class="label">游戏名称</div>
          <div class="ctrl" style="flex: 1; max-width: 340px">
            <input
              v-model="dlPanel.name"
              class="field"
              style="width: 100%"
              :disabled="dlPanel.state !== 'form'"
              :style="dlNameTaken ? { borderColor: '#ff8f8f', color: '#ff8f8f' } : undefined"
            />
            <div v-if="dlNameTaken" class="meta" style="color: #ff8f8f; font-size: 12px">
              已存在同名游戏，请换一个名字
            </div>
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
          <div class="label">下载线程</div>
          <div class="ctrl">
            <select v-model.number="dlPanel.threads" class="field" :disabled="dlPanel.state !== 'form'">
              <option :value="1">1（单连接）</option>
              <option :value="2">2</option>
              <option :value="4">4</option>
              <option :value="8">8</option>
            </select>
            <span class="meta" style="font-size: 12px">
              {{ dlPanel.threads > 1 ? '分段并发，服务器不支持时自动降为 1' : '只用一个连接' }}
            </span>
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
            全部留空即用默认值：下载目录 = 设置里的下载目录 · 线程 = {{ settings.downloadThreads }} ·
            内存 = {{ settings.memory }} MB · 隔离 = {{ settings.saveIsolation ? '开' : '关' }} ·
            Java = java · JVM 参数不加
          </div>
        </div>

        <p v-if="dlPanel.error" style="color: #ff7db0; font-size: 13px; padding: 8px 0">{{ dlPanel.error }}</p>
        <p v-if="dlPanel.state === 'done'" style="color: var(--cyan); font-size: 13px; padding: 8px 0">
          已创建游戏「{{ dlPanel.name }}」
        </p>

        <div class="set-row" style="justify-content: flex-end; gap: 10px">
          <template v-if="dlPanel.state === 'form'">
            <button class="btn-ghost" @click="closePanel">取消</button>
            <button
              class="btn-grad"
              style="font-size: 14px; padding: 9px 26px"
              :disabled="dlNameTaken || !dlPanel.name.trim()"
              @click="confirmDownload"
            >
              下载并创建
            </button>
          </template>
          <template v-else-if="dlPanel.state === 'downloading'">
            <span class="meta">
              下载中 {{ downloadStates[dlPanel.fileName]?.percent?.toFixed(0) ?? 0 }}% ·
              {{ fmtSpeed(downloadStates[dlPanel.fileName]?.speed) }} ·
              {{ downloadStates[dlPanel.fileName]?.threads ?? dlPanel.threads }} 线程
            </span>
            <button class="btn-ghost" @click="viewTasks">查看下载任务</button>
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
