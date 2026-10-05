import { computed, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { GithubVersion, InstanceInfo, InstanceList, VersionIndex } from './types'

export type ViewId = 'home' | 'instances' | 'downloads' | 'mod' | 'settings'
export type LaunchStatus = 'running' | 'done' | 'failed'
export type SettingsTab = 'download' | 'mirror' | 'launch' | 'game' | 'personal'

export const store = reactive({
  view: 'home' as ViewId,
  launch: {
    open: false,
    status: 'running' as LaunchStatus,
    instanceId: '',
    code: null as number | null,
    logs: [] as { text: string; cls: string }[],
  },
})

/** 设置页当前分区（会话态，不持久化） */
export const settingsTab = ref<SettingsTab>('download')

// ---------- 中心索引的版本缓存（开机即取本地，后台再向服务器刷新） ----------
export const centralVersions = ref<VersionIndex | null>(null)
export const versionSyncing = ref(false)

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

/** 后台向服务器索引；内容 hash 未变则不覆盖本地 */
export async function syncCentralVersions(): Promise<void> {
  if (versionSyncing.value) return
  versionSyncing.value = true
  try {
    centralVersions.value = (await invoke('sync_central_versions', {
      base: settings.centralServer.trim(),
      proxy: settings.proxy,
    })) as VersionIndex
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

export function resetLaunch(instanceId: string): void {
  store.launch.open = true
  store.launch.status = 'running'
  store.launch.instanceId = instanceId
  store.launch.code = null
  store.launch.logs = []
}

export function closeLaunch(): void {
  store.launch.open = false
}

// ---------- 下载状态（App 层全局监听，跨页面不丢） ----------
export interface DlState {
  status: 'downloading' | 'done' | 'error'
  percent: number
  received: number
  total: number
  path?: string
  code?: number
}

export const downloadStates = reactive<Record<string, DlState>>({})

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
      proxy: settings.proxy,
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
  proxy: string
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
}

const defaults: Settings = {
  theme: '暗色',
  memory: 4096,
  proxy: '',
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
