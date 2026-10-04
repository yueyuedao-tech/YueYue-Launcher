import { reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { InstanceInfo, InstanceList } from './types'

export type ViewId = 'home' | 'instances' | 'settings'
export type LaunchStatus = 'running' | 'done' | 'failed'

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

// ---------- 实例 ----------
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

// ---------- 扫描结果缓存（每次进页面重扫会造成明显卡顿） ----------
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

// ---------- 设置（localStorage 持久化） ----------
const SETTINGS_KEY = 'starlight-launcher-settings'

export interface Settings {
  theme: '暗色' | '跟随系统'
  memory: number
}

const defaults: Settings = {
  theme: '暗色',
  memory: 4096,
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (raw) return { ...defaults, ...(JSON.parse(raw) as Partial<Settings>) }
  } catch {
    /* 忽略损坏的本地数据 */
  }
  return { ...defaults }
}

export const settings = reactive<Settings>(loadSettings())

watch(
  settings,
  (v) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(v))
  },
  { deep: true },
)
