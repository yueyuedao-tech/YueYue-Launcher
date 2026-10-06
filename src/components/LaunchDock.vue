<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  clearFinishedLaunches,
  closeLaunch,
  closeLogPanel,
  launchDockVisible,
  launchRecords,
  loadInstances,
  logFocusId,
  logPanelOpen,
  markLaunchStopped,
  removeLaunchRecord,
  runningLaunchCount,
  store,
} from '../store'

const logBox = ref<HTMLElement | null>(null)
const stopping = ref(false)
const actionError = ref('')

const records = computed(() =>
  Object.values(launchRecords).sort((a, b) => b.startedAt - a.startedAt),
)
const focused = computed(() => launchRecords[logFocusId.value] ?? records.value[0])

const finishedCount = computed(() => records.value.filter((r) => r.status !== 'running').length)

watch(logPanelOpen, (open) => {
  if (open) {
    actionError.value = ''
    if (!launchRecords[logFocusId.value]) logFocusId.value = records.value[0]?.instanceId ?? ''
    void scrollToEnd()
  }
})

// 新日志到达时贴底（只对当前查看的那条）
watch(
  () => focused.value?.logs.length ?? 0,
  () => void scrollToEnd(),
)

async function scrollToEnd() {
  await nextTick()
  if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight
}

function focus(id: string) {
  logFocusId.value = id
  void scrollToEnd()
}

function statusText(s: string): string {
  return s === 'running' ? '运行中' : s === 'done' ? '已结束' : s === 'stopped' ? '已停止' : '异常退出'
}

function statusStyle(s: string): Record<string, string> {
  if (s === 'running') return { background: 'rgba(110,231,249,.15)', color: 'var(--cyan)' }
  if (s === 'done') return { background: 'rgba(255,255,255,.07)', color: 'var(--ink-dim)' }
  if (s === 'stopped') return { background: 'rgba(255,209,102,.14)', color: 'var(--amber)' }
  return { background: 'rgba(255,120,120,.15)', color: '#ff8f8f' }
}

function duration(r: { startedAt: number; endedAt: number | null }): string {
  const end = r.endedAt ?? Date.now()
  const s = Math.max(0, Math.round((end - r.startedAt) / 1000))
  if (s < 60) return `${s}s`
  if (s < 3600) return `${Math.floor(s / 60)}m${s % 60}s`
  return `${Math.floor(s / 3600)}h${Math.floor((s % 3600) / 60)}m`
}

/** 展开某条记录（浮层只给「正在启动」用，这里用于回看） */
function openRecord(id: string) {
  store.launch.instanceId = id
  store.launch.open = true
  logPanelOpen.value = false
}

async function stop(id: string) {
  stopping.value = true
  actionError.value = ''
  try {
    await invoke('stop_instance', { id })
    markLaunchStopped(id)
    await loadInstances()
  } catch (e) {
    actionError.value = String(e)
  } finally {
    stopping.value = false
  }
}

function close() {
  // 收起面板即视为「看过了」：圆圈随之隐藏，下次启动游戏再出现
  closeLogPanel()
  closeLaunch()
}

/** 点圆圈：展开/收起。收起要走 closeLogPanel，才会标记「已看过」 */
function togglePanel() {
  if (logPanelOpen.value) closeLogPanel()
  else logPanelOpen.value = true
}

function forget(id: string) {
  removeLaunchRecord(id)
  if (!records.value.length) logPanelOpen.value = false
}
</script>

<template>
  <!-- 左下角圆圈：所有启动过的游戏都归档在这里，多开时逐个查看 -->
  <button
    v-if="launchDockVisible"
    class="dock-circle dock-circle--left"
    :class="{ active: logPanelOpen, live: runningLaunchCount > 0 }"
    title="启动日志"
    aria-label="启动日志"
    @click="togglePanel"
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <path d="M5 4h11l3 3v13H5z" />
      <path d="M8 10h8M8 14h8M8 18h5" />
    </svg>
    <i v-if="runningLaunchCount" class="dock-badge">{{ runningLaunchCount }}</i>
  </button>

  <!-- 点面板以外的任何地方都关掉（和下载任务面板一致） -->
  <div v-if="logPanelOpen" class="dock-overlay" @click="closeLogPanel()"></div>

  <div v-if="logPanelOpen" class="dock-panel dock-panel--left" role="dialog" aria-label="启动日志归档">
    <header class="dock-head">
      <div>
        <div class="dock-title">启动日志</div>
        <div class="meta" style="font-size: 12px">
          {{ records.length }} 个游戏<template v-if="runningLaunchCount">
            · {{ runningLaunchCount }} 个运行中</template
          >
        </div>
      </div>
      <div style="display: flex; gap: 8px; align-items: center">
        <button
          v-if="finishedCount"
          class="btn-ghost"
          style="font-size: 12px"
          @click="clearFinishedLaunches"
        >
          清空已结束
        </button>
        <button class="btn-ghost" aria-label="关闭" @click="close">✕</button>
      </div>
    </header>

    <p v-if="actionError" style="color: #ff7db0; font-size: 12px; padding: 0 4px 8px">
      {{ actionError }}
    </p>

    <div class="dock-body">
      <!-- 每个游戏一条：多开时就是多行 -->
      <div class="dock-list">
        <button
          v-for="r in records"
          :key="r.instanceId"
          class="dock-item"
          :class="{ active: focused?.instanceId === r.instanceId }"
          @click="focus(r.instanceId)"
        >
          <div class="dock-item-top">
            <span class="dock-item-name" :title="r.name">{{ r.name }}</span>
            <span class="tag" :style="statusStyle(r.status)">{{ statusText(r.status) }}</span>
          </div>
          <div class="meta" style="font-size: 11.5px">
            {{ r.logs.length }} 行 · {{ duration(r) }}
          </div>
        </button>
      </div>

      <div class="dock-log-wrap">
        <div ref="logBox" class="launch-log dock-log" aria-live="polite">
          <div v-for="(line, i) in focused?.logs ?? []" :key="i" :class="line.cls">
            {{ line.text }}
          </div>
        </div>
        <div v-if="focused" class="dock-actions">
          <button class="btn-ghost" style="font-size: 12px" @click="openRecord(focused.instanceId)">
            用浮层打开
          </button>
          <button
            v-if="focused.status === 'running'"
            class="btn-ghost"
            style="font-size: 12px; color: #ff8f8f"
            :disabled="stopping"
            @click="stop(focused.instanceId)"
          >
            {{ stopping ? '停止中…' : '停止进程' }}
          </button>
          <button
            v-else
            class="btn-ghost"
            style="font-size: 12px"
            @click="forget(focused.instanceId)"
          >
            移除记录
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
