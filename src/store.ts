import { reactive, watch } from 'vue'
import { versions } from './data/mock'

export type ViewId = 'home' | 'versions' | 'downloads' | 'settings'
export type LaunchStatus = 'running' | 'done'

export const store = reactive({
  view: 'home' as ViewId,
  selectedVersionId: versions[0].id,
  launch: {
    open: false,
    progress: 0,
    status: 'running' as LaunchStatus,
    logs: [] as { text: string; cls: string }[],
  },
})

export function selectedVersionName(): string {
  return versions.find((v) => v.id === store.selectedVersionId)?.name ?? versions[0].name
}

const SETTINGS_KEY = 'starlight-launcher-settings'

export interface Settings {
  theme: '暗色' | '跟随系统'
  memory: number
  javaPath: string
  width: number
  height: number
  keepOpen: boolean
  showSnapshots: boolean
}

const defaults: Settings = {
  theme: '暗色',
  memory: 4096,
  javaPath: '自动检测',
  width: 1280,
  height: 800,
  keepOpen: true,
  showSnapshots: true,
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
