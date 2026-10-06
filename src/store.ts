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
  VersionIndex,
} from './types'

export type ViewId = 'home' | 'instances' | 'downloads' | 'mod' | 'settings'
export type LaunchStatus = 'running' | 'done' | 'failed' | 'stopped'
export type SettingsTab = 'download' | 'mirror' | 'launch' | 'game' | 'personal' | 'dev' | 'about'

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

/** 一条日志最多留多少行：长时间挂机的实例不该把内存吃满 */
const MAX_LOG_LINES = 5000

export const runningLaunchCount = computed(
  () => Object.values(launchRecords).filter((r) => r.status === 'running').length,
)

/** 有记录就显示左下角圆圈（归档入口） */
export const launchDockVisible = computed(() => Object.keys(launchRecords).length > 0)

export function launchRecordOf(id: string): LaunchRecord | undefined {
  return launchRecords[id]
}

export function resetLaunch(instanceId: string, name = ''): void {
  const info = instances.value.find((i) => i.id === instanceId)
  store.launch.open = true
  store.launch.instanceId = instanceId
  logFocusId.value = instanceId
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
let centralLoadedFor: string | null = null

/** 拉中心索引。默认「同一个服务器地址只拉一次」；force=true 强制重拉。 */
export async function loadCentralIndex(force = false): Promise<void> {
  if (centralLoading.value) return
  const base = settings.centralServer.trim()
  if (!force && centralLoadedFor === base && centralItems.value.length) return
  centralLoading.value = true
  try {
    const r = (await invoke('fetch_central_index', {
      base,
      proxy: effectiveProxy.value,
    })) as CentralIndex
    centralItems.value = r.items
    centralMeta.value = { source: r.source, note: r.note }
    if (r.mirrors?.workshop?.length) workshopMirrors.value = r.mirrors.workshop
    centralLoadedFor = base
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
    centralVersions.value = (await invoke('sync_central_versions', {
      base: settings.centralServer.trim(),
      proxy: effectiveProxy.value,
      force,
    })) as VersionIndex
    if (centralVersions.value?.mirrors?.workshop?.length) {
      workshopMirrors.value = centralVersions.value.mirrors.workshop
    }
  } catch (e) {
    console.error('syncCentralVersions failed', e)
  } finally {
    versionSyncing.value = false
  }
}

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

// ---------- 设置（localStorage 持久化） ----------
const SETTINGS_KEY = 'starlight-launcher-settings'

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
  downloadDir: string
  updateCheck: boolean
  lastSeenTag: string
  navPosition: 'left' | 'top'
  /** 设置页分类导航位置；不填 = 跟随主导航 */
  settingsNavPosition?: 'left' | 'top'
  navMigrated?: boolean
  /** 中心化服务器地址（留空 = 用内置索引） */
  centralServer: string
  workshopMirror: string
  /** 下载线程数：1 = 单连接（服务器不支持分段时自动降为 1） */
  downloadThreads: number
  /** 开发者模式：打开后才显示还没写完的功能（当前是 Mod 页） */
  devMode: boolean
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
  /** 游戏进程退出时自动弹出该实例的日志 */
  autoOpenLogOnExit: boolean
  /** 启动器打开时停在哪个页面 */
  startView: ViewId
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
  downloadDir: '',
  updateCheck: true,
  lastSeenTag: '',
  navPosition: 'top',
  centralServer: '',
  workshopMirror: '',
  downloadThreads: 4,
  devMode: false,
  uiScale: 100,
  defaultJvmArgs: '',
  defaultGameArgs: '',
  downloadNotify: true,
  bgBlur: 0,
  animations: true,
  autoOpenLogOnExit: true,
  startView: 'home',
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (raw) {
      const parsed = { ...defaults, ...(JSON.parse(raw) as Partial<Settings>) } as Settings
      // 一次性迁移：老配置升级后默认改为顶部导航
      if (!parsed.navMigrated) {
        parsed.navPosition = 'top'
        parsed.navMigrated = true
      }
      return parsed
    }
  } catch {
    /* 忽略损坏的本地数据 */
  }
  return { ...defaults, navMigrated: true }
}

export const settings = reactive<Settings>(loadSettings())

/** 设置页分类导航位置：没单独设置过就跟随主导航 */
export const settingsNavPos = computed<'left' | 'top'>(
  () => settings.settingsNavPosition ?? settings.navPosition,
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

/** 从本地版本缓存里取上次下发的镜像（中心服务器临时不可达时设置页也不空） */
export async function loadCachedMirrors(): Promise<void> {
  const idx = centralVersions.value
  if (idx?.mirrors?.workshop?.length) workshopMirrors.value = idx.mirrors.workshop
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
