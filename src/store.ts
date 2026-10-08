import { computed, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type {
  CentralIndex,
  CentralItem,
  DownloadTask,
  GithubVersion,
  InstanceInfo,
  InstanceList,
  LauncherInfo,
  VersionIndex,
} from './types'
import { defaultInfoBar } from './data/mock'

export type ViewId = 'home' | 'instances' | 'downloads' | 'maps' | 'mod' | 'rooms' | 'settings'
export type LaunchStatus = 'running' | 'done' | 'failed' | 'stopped'
export type SettingsTab = 'download' | 'mirror' | 'launch' | 'game' | 'personal' | 'account' | 'about'

export const store = reactive({
  view: 'home' as ViewId,
  /** 启动浮层：只表示「现在弹不弹」，具体内容看 launchRecords[instanceId] */
  launch: {
    open: false,
    instanceId: '',
  },
})

/* ---------- 启动日志：按实例归档，多开时各看各的 ---------- */

export interface LaunchLine {
  text: string
  cls: string
}

export interface LaunchRecord {
  instanceId: string
  name: string
  status: LaunchStatus
  code: number | null
  logs: LaunchLine[]
  startedAt: number
  endedAt: number | null
}

/** 实例 id → 启动记录。多开时每条游戏各自一份日志，互不覆盖。 */
export const launchRecords = reactive<Record<string, LaunchRecord>>({})
/** 左下角日志面板是否展开 */
export const logPanelOpen = ref(false)
/** 面板里当前查看哪个实例的日志 */
export const logFocusId = ref('')
/** 玩家已经看过这次启动的日志：看过之后圆圈就收起来，下次启动游戏再出现 */
export const logSeen = ref(false)

/** 一条日志最多留多少行：长时间挂机的实例不该把内存吃满 */
const MAX_LOG_LINES = 5000

export const runningLaunchCount = computed(
  () => Object.values(launchRecords).filter((r) => r.status === 'running').length,
)

/** 有记录、且玩家还没看过这次日志 → 显示左下角圆圈（归档入口） */
export const launchDockVisible = computed(
  () => Object.keys(launchRecords).length > 0 && !logSeen.value,
)

/** 收起日志面板：看过一次就标记，圆圈随之隐藏 */
export function closeLogPanel(): void {
  if (!logPanelOpen.value) return
  logPanelOpen.value = false
  logSeen.value = true
}

export function launchRecordOf(id: string): LaunchRecord | undefined {
  return launchRecords[id]
}

export function resetLaunch(instanceId: string, name = ''): void {
  const info = instances.value.find((i) => i.id === instanceId)
  store.launch.open = true
  store.launch.instanceId = instanceId
  logFocusId.value = instanceId
  // 新一次启动 → 圆圈重新出现
  logSeen.value = false
  launchRecords[instanceId] = {
    instanceId,
    name: name || info?.name || instanceId,
    status: 'running',
    code: null,
    logs: [],
    startedAt: Date.now(),
    endedAt: null,
  }
  appendLaunchLog(instanceId, `[启动器] 请求启动游戏「${launchRecords[instanceId].name}」…`)
}

export function appendLaunchLog(instanceId: string, text: string, cls = ''): void {
  const r = launchRecords[instanceId]
  if (!r) return
  r.logs.push({ text, cls })
  if (r.logs.length > MAX_LOG_LINES) r.logs.splice(0, r.logs.length - MAX_LOG_LINES)
}

/** 进程退出（后端 launch-exit）：写入状态与收尾日志 */
export function setLaunchExit(instanceId: string, code: number): void {
  const r = launchRecords[instanceId]
  if (!r) return
  r.code = code
  r.status = code === 0 ? 'done' : 'failed'
  r.endedAt = Date.now()
  appendLaunchLog(
    instanceId,
    code === 0 ? '[进程] 已退出，code=0' : `[进程] 异常退出，code=${code}`,
    code === 0 ? 'ok' : 'warn',
  )
}

/** 用户主动停止：后端对被 stop 的实例不发 launch-exit，状态要在这里落定 */
export function markLaunchStopped(instanceId: string): void {
  const r = launchRecords[instanceId]
  if (!r || r.status !== 'running') return
  r.status = 'stopped'
  r.endedAt = Date.now()
  appendLaunchLog(instanceId, '[启动器] 已停止进程', 'warn')
}

/** 拉起进程本身失败（jar 不存在 / java 找不到等）：记录里落失败态 */
export function markLaunchFailed(instanceId: string, message: string): void {
  const r = launchRecords[instanceId]
  if (!r) return
  r.status = 'failed'
  r.endedAt = Date.now()
  appendLaunchLog(instanceId, `[错误] ${message}`, 'warn')
}

/** 打开某实例的启动日志：已有归档就复用，没有才新建（重复打开不该清空日志） */
export function openLaunchLog(instanceId: string): void {
  if (launchRecords[instanceId]) {
    store.launch.instanceId = instanceId
    store.launch.open = true
    logFocusId.value = instanceId
  } else {
    resetLaunch(instanceId)
  }
}

/** 启动成功后自动把浮层收起：日志继续写进左下角归档，不再挡住界面 */
export function autoCollapseLaunch(instanceId: string, delayMs = 2000): void {
  setTimeout(() => {
    if (store.launch.open && store.launch.instanceId === instanceId) store.launch.open = false
  }, delayMs)
}

export function removeLaunchRecord(instanceId: string): void {
  delete launchRecords[instanceId]
  if (logFocusId.value === instanceId) {
    logFocusId.value = Object.keys(launchRecords)[0] ?? ''
  }
}

export function clearFinishedLaunches(): void {
  for (const id of Object.keys(launchRecords)) {
    if (launchRecords[id].status !== 'running') delete launchRecords[id]
  }
  if (!launchRecords[logFocusId.value]) {
    logFocusId.value = Object.keys(launchRecords)[0] ?? ''
  }
}

/** 设置页当前分区（会话态，不持久化） */
export const settingsTab = ref<SettingsTab>('download')

// ---------- 中心索引的版本缓存（开机即取本地，后台再向服务器刷新） ----------
export const centralVersions = ref<VersionIndex | null>(null)
export const versionSyncing = ref(false)

/** 中心源清单（名称 / logo / 标语 / 标签 / 版本快照）：开机就拉，进下载页不必再等索引 */
export const centralItems = ref<CentralItem[]>([])
export const centralMeta = ref({ source: '', note: '' })
export const centralLoading = ref(false)
/** 本次实际用上的中心化服务器（列表里第一个可用的） */
export const centralServerUsed = ref('')
let centralLoadedFor: string | null = null

/** 设置里配的服务器列表（去空、去首尾空格） */
export function centralServerList(): string[] {
  return (settings.centralServers ?? []).map((s) => s.trim()).filter(Boolean)
}

/** 依次尝试列表里的每个服务器，返回第一个满足 ok 的结果；都不行返回 null */
async function pickCentralServer<T>(
  run: (base: string) => Promise<T>,
  ok: (r: T) => boolean,
): Promise<{ base: string; result: T } | null> {
  for (const base of centralServerList()) {
    try {
      const result = await run(base)
      if (ok(result)) return { base, result }
    } catch {
      /* 这个不行，换下一个 */
    }
  }
  return null
}

/** 拉中心索引。默认「同一份服务器列表只拉一次」；force=true 强制重拉。 */
export async function loadCentralIndex(force = false): Promise<void> {
  if (centralLoading.value) return
  const list = centralServerList()
  const key = list.join('|')
  if (!force && centralLoadedFor === key && centralItems.value.length) return
  centralLoading.value = true
  try {
    const hit = await pickCentralServer(
      (base) =>
        invoke('fetch_central_index', {
          base,
          proxy: effectiveProxy.value,
        }) as Promise<CentralIndex>,
      (r) => r.source === 'remote' && r.items.length > 0,
    )
    if (hit) {
      centralItems.value = hit.result.items
      centralMeta.value = { source: 'remote', note: '' }
      centralServerUsed.value = hit.base
      if (hit.result.mirrors?.workshop?.length) workshopMirrors.value = hit.result.mirrors.workshop
      if (hit.result.infoBar) infoBarText.value = hit.result.infoBar
    } else {
      // 列表为空，或全都连不上 → 内置索引兜底
      const r = (await invoke('fetch_central_index', {
        base: '',
        proxy: effectiveProxy.value,
      })) as CentralIndex
      centralItems.value = r.items
      centralMeta.value = {
        source: 'builtin',
        note: list.length ? '所有中心化服务器都不可达；已用内置索引' : r.note,
      }
      centralServerUsed.value = ''
    }
    centralLoadedFor = key
  } catch (e) {
    centralMeta.value = { source: 'builtin', note: String(e) }
    centralItems.value = []
  } finally {
    centralLoading.value = false
  }
}

export function countCentralVersions(): number {
  const src = centralVersions.value?.sources ?? {}
  return Object.values(src).reduce((a, b) => a + b.length, 0)
}

/** 只读本地缓存：界面立刻有内容，不等网络 */
export async function loadCentralVersions(): Promise<void> {
  try {
    centralVersions.value = (await invoke('list_central_versions')) as VersionIndex
  } catch {
    centralVersions.value = null
  }
}

/** 后台向服务器索引；内容 hash 未变则不覆盖本地。force = 忽略 15 分钟缓存强制重同步 */
export async function syncCentralVersions(force = false): Promise<void> {
  if (versionSyncing.value) return
  versionSyncing.value = true
  try {
    // 服务器列表按顺序回退：第一个能给出 ok>0 的就用它
    const hit = await pickCentralServer(
      (base) =>
        invoke('sync_central_versions', {
          base,
          proxy: effectiveProxy.value,
          force,
        }) as Promise<VersionIndex>,
      (r) => r.ok > 0,
    )
    if (hit) {
      centralVersions.value = hit.result
      centralServerUsed.value = hit.base
    } else {
      centralVersions.value = (await invoke('sync_central_versions', {
        base: '',
        proxy: effectiveProxy.value,
        force,
      })) as VersionIndex
    }
    if (centralVersions.value?.mirrors?.workshop?.length) {
      workshopMirrors.value = centralVersions.value.mirrors.workshop
    }
    if (centralVersions.value?.infoBar) infoBarText.value = centralVersions.value.infoBar
  } catch (e) {
    console.error('syncCentralVersions failed', e)
  } finally {
    versionSyncing.value = false
  }
}

// 服务器列表一改就重新拉索引与版本（防抖，避免在输入框里打字时狂发请求）。
// 没有这个的话，改完地址要重启应用才生效，很容易误以为「服务器不可达」。
let serverChangeTimer: ReturnType<typeof setTimeout> | undefined
watch(
  () => centralServerList().join('|'),
  () => {
    if (serverChangeTimer) clearTimeout(serverChangeTimer)
    serverChangeTimer = setTimeout(() => {
      void loadCentralIndex(true)
      void syncCentralVersions(true)
    }, 900)
  },
)

// ---------- 游戏 ----------
export const instances = ref<InstanceInfo[]>([])
export const skippedCount = ref(0)
export const selectedInstanceId = ref('')

export async function loadInstances(): Promise<void> {
  try {
    const r = (await invoke('list_instances')) as InstanceList
    instances.value = r.items
    skippedCount.value = r.skipped
    if (!instances.value.some((i) => i.id === selectedInstanceId.value)) {
      selectedInstanceId.value = instances.value[0]?.id ?? ''
    }
  } catch (e) {
    console.error('loadInstances failed', e)
  }
}

/** 已隐藏（删本体、保留存档）的游戏：不参与正常列表，可恢复 */
export const hiddenInstances = ref<InstanceInfo[]>([])

export async function loadHidden(): Promise<void> {
  try {
    hiddenInstances.value = (await invoke('list_hidden_instances')) as InstanceInfo[]
  } catch (e) {
    console.error('loadHidden failed', e)
    hiddenInstances.value = []
  }
}

// ---------- 扫描结果缓存 ----------
export const jarScan = ref<string[]>([])
export const javaScan = ref<string[]>(['java'])
let scanDone = false

export async function ensureScanned(force = false): Promise<void> {
  if (scanDone && !force) return
  scanDone = true
  try {
    jarScan.value = (await invoke('scan_jars')) as string[]
    javaScan.value = (await invoke('scan_javas')) as string[]
  } catch (e) {
    console.error('scan failed', e)
  }
}

export function currentInstance(): InstanceInfo | undefined {
  return instances.value.find((i) => i.id === selectedInstanceId.value)
}

/** 收起启动浮层（记录仍留在左下角圆圈里） */
export function closeLaunch(): void {
  store.launch.open = false
}

// ---------- 下载状态（App 层全局监听，跨页面不丢） ----------
export interface DlState {
  status: 'downloading' | 'done' | 'error'
  percent: number
  received: number
  total: number
  /** 字节/秒；仅下载中由后端推送 */
  speed?: number
  /** 实际连接数 */
  threads?: number
  path?: string
  code?: number
  /** 这条不是下载来的，而是直接复用了本地已有的客户端文件 */
  reused?: boolean
}

export const downloadStates = reactive<Record<string, DlState>>({})

// ---------- 下载任务抽屉 ----------
/** 顶栏下载图标点开的右侧任务面板 */
export const taskPanelOpen = ref(false)
export const downloadTasks = ref<DownloadTask[]>([])
/** 还有几条在跑（顶栏角标用） */
export const activeDownloadCount = computed(
  () => downloadTasks.value.filter((t) => t.status === 'downloading').length,
)

/** 面板打开时拉一次后端任务表：只靠事件会漏掉状态 */
export async function refreshDownloadTasks(): Promise<void> {
  try {
    const list = (await invoke('list_downloads')) as DownloadTask[]
    downloadTasks.value = list.filter((t) => !canceledTasks.has(t.fileName))
  } catch {
    /* 拉不到就保持现状，事件仍会继续更新 */
  }
}

/** 已取消的文件名：取消后后端仍可能有在途的 progress 事件，不能让它把任务又插回列表 */
const canceledTasks = new Set<string>()

export function markDownloadCanceled(fileName: string): void {
  canceledTasks.add(fileName)
}

export function unmarkDownloadCanceled(fileName: string): void {
  canceledTasks.delete(fileName)
}

/** 用事件增量更新某条任务（事件里没有 name/url，所以只补进度字段）。
 *  事件可能先于刷新到达：找不到就先建一条最小记录，角标与抽屉立刻能看到。 */
export function patchDownloadTask(
  fileName: string,
  patch: Partial<DownloadTask>,
): void {
  if (canceledTasks.has(fileName)) return
  const t = downloadTasks.value.find((x) => x.fileName === fileName)
  if (t) {
    Object.assign(t, patch)
    return
  }
  if (patch.status === 'downloading' || patch.status === 'done' || patch.status === 'error') {
    downloadTasks.value.unshift({
      fileName,
      name: fileName,
      url: '',
      status: 'downloading',
      received: 0,
      total: 0,
      percent: 0,
      speed: 0,
      threads: 0,
      path: '',
      code: 0,
      startedAt: Date.now(),
      ...patch,
    })
  }
}

export async function clearDownloadTask(fileName: string): Promise<string> {
  try {
    await invoke('clear_download', { fileName })
    downloadTasks.value = downloadTasks.value.filter((t) => t.fileName !== fileName)
    return ''
  } catch (e) {
    return String(e)
  }
}

export async function clearFinishedDownloads(): Promise<void> {
  try {
    await invoke('clear_finished_downloads')
    await refreshDownloadTasks()
  } catch {
    /* 忽略 */
  }
}

// ---------- 新版本小红点 ----------
export const updateDot = ref(false)
export const latestTag = ref('')

/** 启动后静默检查 GitHub 最新版本（失败静默） */
export async function checkForUpdates(): Promise<void> {
  if (!settings.updateCheck) return
  try {
    const versions = (await invoke('fetch_repo_versions', {
      repo: 'Anuken/Mindustry',
      asset: 'Mindustry.jar',
      proxy: effectiveProxy.value,
      prefix: settings.githubPrefix,
    })) as GithubVersion[]
    if (versions.length) {
      latestTag.value = versions[0].tag
      updateDot.value = latestTag.value !== settings.lastSeenTag
    }
  } catch {
    /* 离线/被墙 → 静默 */
  }
}

/** 进入下载页视为已读 */
export function ackUpdate(): void {
  if (updateDot.value && latestTag.value) {
    settings.lastSeenTag = latestTag.value
  }
  updateDot.value = false
}

/* ---------- 启动器自身的版本检查（设置 → 关于） ---------- */

export type UpdateState = 'idle' | 'checking' | 'latest' | 'newer' | 'failed'

/** 中心化服务器告诉我们的最新版本 */
export const launcherLatest = ref<LauncherInfo | null>(null)
export const updateState = ref<UpdateState>('idle')
export const updateError = ref('')

/** 版本号比较：a > b 返回 1，相等 0，小于 -1（只按数字段比，够用） */
export function compareVersion(a: string, b: string): number {
  const pa = a.replace(/^v/i, '').split(/[.\-+]/).map((x) => parseInt(x, 10) || 0)
  const pb = b.replace(/^v/i, '').split(/[.\-+]/).map((x) => parseInt(x, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? 0
    const y = pb[i] ?? 0
    if (x !== y) return x > y ? 1 : -1
  }
  return 0
}

/**
 * 「检查更新」：现拉一次中心化索引，读里面下发的启动器版本再和当前版本比。
 * 现拉而不是用缓存，是因为这个按钮点了就该是「现在的最新情况」。
 */
export async function checkLauncherUpdate(current: string): Promise<void> {
  updateState.value = 'checking'
  updateError.value = ''
  launcherLatest.value = null
  try {
    const hit = await pickCentralServer(
      (base) =>
        invoke('fetch_central_index', {
          base,
          proxy: effectiveProxy.value,
        }) as Promise<CentralIndex>,
      (r) => r.source === 'remote',
    )
    const idx =
      hit?.result ??
      ((await invoke('fetch_central_index', {
        base: '',
        proxy: effectiveProxy.value,
      })) as CentralIndex)
    const info = idx.launcher
    if (!info?.version) {
      updateState.value = 'failed'
      updateError.value = '中心化服务器没有提供版本信息（可在服务端 content/launcher.json 里配置）'
      return
    }
    launcherLatest.value = info
    updateState.value = compareVersion(info.version, current) > 0 ? 'newer' : 'latest'
  } catch (e) {
    updateState.value = 'failed'
    updateError.value = String(e)
  }
}

// ---------- 设置（localStorage 持久化） ----------
const SETTINGS_KEY = 'starlight-launcher-settings'
export const DEFAULT_REMOTE_PROXY = 'https://mindustry.wiki:1200'

export const ACCENT_PAIRS: Record<string, [string, string]> = {
  粉: ['#ff7db0', '#6ee7f9'],
  紫: ['#c084fc', '#818cf8'],
  青: ['#22d3ee', '#a78bfa'],
  橙: ['#fb923c', '#fbbf24'],
}

export interface Settings {
  theme: '暗色' | '跟随系统'
  memory: number
  /** 自定义代理地址（proxyMode = 'custom' 时生效） */
  proxy: string
  /** 代理来源：system = 读系统代理；custom = 用下面填的地址 */
  proxyMode: 'system' | 'custom'
  githubPrefix: string
  accent: keyof typeof ACCENT_PAIRS | string
  bgShade: number
  alwaysOnTop: boolean
  closeBehavior: 'exit' | 'minimize'
  /** 删除游戏的默认方式：keep = 删本体留存档；purge = 连存档一起删 */
  defaultDelete: 'keep' | 'purge'
  /** 是否启用存档隔离（启动设置） */
  saveIsolation: boolean
  /** 自定义背景图，dataURL；空 = 默认背景 */
  bgImage: string
  /** 背景图是否也用在其他页面（首页始终显示） */
  bgAllPages: boolean
  /** 导航栏透明度：0 = 完全不透明，100 = 全透明（默认 28 ≈ 原来的 0.72 不透明度） */
  navTransparency: number
  downloadDir: string
  updateCheck: boolean
  lastSeenTag: string
  navPosition: 'left' | 'top'
  /** 设置页分类导航位置；不填 = 跟随主导航 */
  settingsNavPosition?: 'left' | 'top'
  navMigrated?: boolean
  /** 中心化服务器列表：按顺序尝试，前面的连不上就自动用后面的（空 = 用内置索引） */
  centralServers: string[]
  /** 可选的中心服务器代理入口，负责转发地图、Mod 索引和下载流量 */
  remoteProxy: string
  remoteProxyDefaultApplied?: boolean
  /** 使用自定义 Mod 索引；关闭时走内置 MindustryMods 清单 */
  modIndexEnabled: boolean
  modIndexUrl: string
  /** 旧字段（单个地址），只在迁移时读一次 */
  centralServer?: string
  workshopMirror: string
  /** 下载线程数：1 = 单连接（服务器不支持分段时自动降为 1） */
  downloadThreads: number
  /** 界面缩放百分比（100 = 原始大小） */
  uiScale: number
  /** 新建游戏时默认带入的 JVM 参数 */
  defaultJvmArgs: string
  /** 新建游戏时默认带入的游戏参数 */
  defaultGameArgs: string
  /** 下载完成时提醒（任务栏闪烁 + 系统通知） */
  downloadNotify: boolean
  /** 主页背景模糊半径（px） */
  bgBlur: number
  /** 界面动画开关 */
  animations: boolean
  /** 启动器打开时停在哪个页面 */
  startView: ViewId
  /** 每日信息（首页公告）：开关，默认开启 */
  infoBarEnabled: boolean
  /** 每日信息的来源链接；留空则用中心化服务器下发的内容 */
  infoBarUrl: string
  /** 地图列表元数据缓存有效期（分钟，最高 60） */
  mapCacheMinutes: number
}

const defaults: Settings = {
  theme: '暗色',
  memory: 4096,
  proxy: '',
  // 默认沿用「自定义（留空=直连）」，保持老行为；要用系统代理得显式切
  proxyMode: 'custom',
  githubPrefix: '',
  accent: '粉',
  bgShade: 96,
  alwaysOnTop: false,
  closeBehavior: 'exit',
  defaultDelete: 'keep',
  saveIsolation: true,
  bgImage: '',
  bgAllPages: true,
  navTransparency: 28,
  downloadDir: '',
  updateCheck: true,
  lastSeenTag: '',
  navPosition: 'top',
  centralServers: [],
  remoteProxy: DEFAULT_REMOTE_PROXY,
  remoteProxyDefaultApplied: true,
  modIndexEnabled: false,
  modIndexUrl: 'https://raw.githubusercontent.com/Anuken/MindustryMods/master/mods.json',
  workshopMirror: '',
  downloadThreads: 4,
  uiScale: 100,
  defaultJvmArgs: '',
  defaultGameArgs: '',
  downloadNotify: true,
  bgBlur: 0,
  animations: true,
  startView: 'home',
  infoBarEnabled: true,
  infoBarUrl: '',
  mapCacheMinutes: 60,
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (raw) {
      const stored = JSON.parse(raw) as Partial<Settings>
      const parsed = { ...defaults, ...stored } as Settings
      if (!stored.remoteProxyDefaultApplied) {
        if (!stored.remoteProxy?.trim()) parsed.remoteProxy = DEFAULT_REMOTE_PROXY
        parsed.remoteProxyDefaultApplied = true
      }
      // 旧版设置中的开发者开关已经移除。
      delete (parsed as Settings & { devMode?: boolean }).devMode
      // 一次性迁移：老配置升级后默认改为顶部导航
      if (!parsed.navMigrated) {
        parsed.navPosition = 'top'
        parsed.navMigrated = true
      }
      // 一次性迁移：单个「中心化服务器」→ 服务器列表。
      // 必须看**原始**对象：合并 defaults 之后 centralServers 永远是个数组，判断会失效。
      if (!Array.isArray(stored.centralServers)) {
        const old = (stored.centralServer ?? '').trim()
        parsed.centralServers = old ? [old] : []
      }
      return parsed
    }
  } catch {
    /* 忽略损坏的本地数据 */
  }
  return { ...defaults, navMigrated: true }
}

export const settings = reactive<Settings>(loadSettings())

export function remoteProxyUrl(url: string): string {
  const base = settings.remoteProxy.trim().replace(/\/+$/, '')
  return base ? `${base}/proxy?url=${encodeURIComponent(url)}` : url
}

/** 设置页分类导航位置：没单独设置过就跟随主导航 */
export const settingsNavPos = computed<'left' | 'top'>(
  () => settings.settingsNavPosition ?? settings.navPosition,
)

/**
 * 背景图层的内联样式：选了自定义图就用它，没选则返回 undefined
 * （走 CSS 里的内置 hero 图）。首页与其他页面共用同一份，保证是同一张图。
 */
export const bgImageStyle = computed(() =>
  settings.bgImage
    ? {
        backgroundImage: `url("${settings.bgImage}")`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }
    : undefined,
)

watch(
  settings,
  (v) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(v))
  },
  { deep: true },
)

function applyAccent(): void {
  const pair = ACCENT_PAIRS[settings.accent] ?? ACCENT_PAIRS['粉']
  document.documentElement.style.setProperty('--pink', pair[0])
  document.documentElement.style.setProperty('--cyan', pair[1])
}

function applyBgShade(): void {
  // 遮罩强度 0-100 → 0.05-0.97（96 ≈ 原默认 0.93）
  const veil = Math.min(0.97, Math.max(0.05, (settings.bgShade / 100) * 0.97))
  document.documentElement.style.setProperty('--veil', veil.toFixed(3))
}

watch(() => settings.accent, applyAccent, { immediate: true })
watch(() => settings.bgShade, applyBgShade, { immediate: true })

/** 导航栏透明度 0-100 → 不透明度 1.000-0.000 */
function applyNavTransparency(): void {
  const t = Math.min(100, Math.max(0, Number(settings.navTransparency) || 0))
  document.documentElement.style.setProperty('--nav-alpha', (1 - t / 100).toFixed(3))
}

watch(() => settings.navTransparency, applyNavTransparency, { immediate: true })

/* ---------- 代理：系统代理 / 自定义 ---------- */

/** 系统代理（读环境变量或 Windows WinINET 设置），开机取一次 */
export const systemProxy = ref('')

export async function loadSystemProxy(): Promise<void> {
  try {
    systemProxy.value = ((await invoke('get_system_proxy')) as string) ?? ''
  } catch {
    systemProxy.value = ''
  }
}

/** 真正传给下载/索引请求的代理口径 */
export const effectiveProxy = computed(() =>
  settings.proxyMode === 'system' ? systemProxy.value.trim() : settings.proxy.trim(),
)

/** 设置页展示用：当前实际在用哪个代理 */
export const effectiveProxyLabel = computed(() => {
  if (settings.proxyMode === 'system') {
    return systemProxy.value ? `系统代理 ${systemProxy.value}` : '系统未设置代理（直连）'
  }
  return settings.proxy.trim() ? `自定义 ${settings.proxy.trim()}` : '直连（未设代理）'
})

/* ---------- 中心下发的镜像清单 ---------- */

export interface MirrorItem {
  name: string
  url: string
}

export const workshopMirrors = ref<MirrorItem[]>([])

/** 每日信息：中心化服务器下发的 Markdown 原文 */
export const infoBarText = ref('')
/** 每日信息：按设置里那个链接拉到的 Markdown 原文 */
export const infoBarRemote = ref('')
export const infoBarLoading = ref(false)
export const infoBarError = ref('')

/**
 * 每日信息最终生效的内容，优先级：
 *   设置里那个链接拉到的 → 中心化服务器下发的 → 内置默认（原来写死的三条）
 * 开关关掉则整块不显示。
 */
export const effectiveInfoBar = computed(() => {
  if (!settings.infoBarEnabled) return ''
  return infoBarRemote.value || infoBarText.value || defaultInfoBar
})

/** 按设置里的链接拉每日信息；开关关掉或没填链接则清空 */
export async function loadInfoBarRemote(): Promise<void> {
  const url = settings.infoBarUrl.trim()
  if (!settings.infoBarEnabled || !url) {
    infoBarRemote.value = ''
    infoBarError.value = ''
    return
  }
  infoBarLoading.value = true
  infoBarError.value = ''
  try {
    infoBarRemote.value = (await invoke('fetch_text', {
      url,
      proxy: effectiveProxy.value,
    })) as string
  } catch (e) {
    infoBarRemote.value = ''
    infoBarError.value = String(e)
  } finally {
    infoBarLoading.value = false
  }
}

// 链接或开关一变就重新拉：开关重新打开即刻生效
watch(
  [() => settings.infoBarEnabled, () => settings.infoBarUrl],
  () => {
    void loadInfoBarRemote()
  },
)

/** 从本地版本缓存里取上次下发的镜像与每日信息（中心服务器临时不可达时也不会空） */
export async function loadCachedMirrors(): Promise<void> {
  const idx = centralVersions.value
  if (idx?.mirrors?.workshop?.length) workshopMirrors.value = idx.mirrors.workshop
  if (idx?.infoBar) infoBarText.value = idx.infoBar
}

/* ---------- 窗口相关设置：必须在窗口就绪后再应用 ---------- */

/** 模块加载时的 watch(immediate) 可能早于窗口就绪，调用会被 catch 吞掉，
 *  表现就是「重启后置顶/关闭行为不生效」。开机时在 App 挂载后再落一次。 */
export async function applyWindowSettings(): Promise<void> {
  try {
    await getCurrentWindow().setAlwaysOnTop(settings.alwaysOnTop)
  } catch (e) {
    console.error('setAlwaysOnTop failed', e)
  }
  try {
    await invoke('set_close_behavior', { mode: settings.closeBehavior })
  } catch (e) {
    console.error('set_close_behavior failed', e)
  }
}

watch(
  () => settings.alwaysOnTop,
  (v) => {
    getCurrentWindow()
      .setAlwaysOnTop(v)
      .catch((e) => console.error('setAlwaysOnTop failed', e))
  },
  { immediate: true },
)

watch(
  () => settings.closeBehavior,
  (v) => {
    invoke('set_close_behavior', { mode: v }).catch((e) =>
      console.error('set_close_behavior failed', e),
    )
  },
  { immediate: true },
)

/* ---------- 界面缩放 / 背景模糊 / 动画 ---------- */

function applyUiScale(): void {
  const pct = Math.min(200, Math.max(80, Number(settings.uiScale) || 100))
  // WebView2(Chromium) 支持根元素 zoom；整页等比缩放，固定定位的圆圈也跟着走
  document.documentElement.style.zoom = String(pct / 100)
}

function applyBgBlur(): void {
  const px = Math.min(40, Math.max(0, Number(settings.bgBlur) || 0))
  document.documentElement.style.setProperty('--bg-blur', `${px}px`)
}

function applyAnimations(): void {
  document.documentElement.classList.toggle('no-anim', !settings.animations)
}

watch(() => settings.uiScale, applyUiScale, { immediate: true })
watch(() => settings.bgBlur, applyBgBlur, { immediate: true })
watch(() => settings.animations, applyAnimations, { immediate: true })

/* ---------- 下载完成提醒 ---------- */

export async function notifyDownloadDone(label: string): Promise<void> {
  if (!settings.downloadNotify) return
  // 任务栏闪烁（不依赖任何插件/权限）
  try {
    await getCurrentWindow().requestUserAttention(2)
  } catch {
    /* 忽略 */
  }
  try {
    const N = (window as unknown as { Notification?: typeof Notification }).Notification
    if (N && N.permission === 'granted') {
      new N('下载完成', { body: label })
    }
  } catch {
    /* 系统通知失败不影响下载 */
  }
}

/** 打开「下载完成提醒」时申请一次通知权限（失败就用任务栏闪烁兜底） */
export async function ensureNotifyPermission(): Promise<void> {
  try {
    const N = (window as unknown as { Notification?: typeof Notification }).Notification
    if (N && N.permission === 'default') await N.requestPermission()
  } catch {
    /* 忽略 */
  }
}
