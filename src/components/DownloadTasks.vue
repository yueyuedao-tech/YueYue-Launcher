<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  activeDownloadCount,
  clearDownloadTask,
  clearFinishedDownloads,
  downloadStates,
  downloadTasks,
  markDownloadCanceled,
  refreshDownloadTasks,
  taskPanelOpen,
} from '../store'
import type { DownloadTask } from '../types'

const actionError = ref('')

const finishedCount = computed(
  () => downloadTasks.value.filter((t) => t.status !== 'downloading').length,
)

// 打开时拉一次后端任务表：只靠事件会漏掉状态（比如刚启动进面板）
watch(taskPanelOpen, (open) => {
  if (open) {
    actionError.value = ''
    void refreshDownloadTasks()
  }
})

function fmtBytes(n: number): string {
  if (!n || n < 0) return '0 B'
  if (n >= 1 << 30) return (n / (1 << 30)).toFixed(2) + ' GB'
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + ' MB'
  if (n >= 1 << 10) return (n / (1 << 10)).toFixed(0) + ' KB'
  // 必须取整：速度很小时这里会打出 0.00000000 这种一长串小数
  return Math.round(n) + ' B'
}

function fmtSpeed(bps: number): string {
  return bps > 0 ? fmtBytes(bps) + '/s' : '测速中…'
}

function statusText(s: string, reused?: boolean): string {
  if (reused) return '已复用'
  return s === 'done' ? '已完成' : s === 'error' ? '失败' : '下载中'
}

function statusStyle(s: string, reused?: boolean): Record<string, string> {
  if (reused) return { background: 'rgba(255,255,255,.08)', color: 'var(--ink-dim)' }
  if (s === 'done') return { background: 'rgba(110,231,249,.15)', color: 'var(--cyan)' }
  if (s === 'error') return { background: 'rgba(255,120,120,.15)', color: '#ff8f8f' }
  return { background: 'rgba(255,125,176,.16)', color: 'var(--pink)' }
}

/** 取消：与下载页「取消」同语义——后端停掉 + 前端复位，任务从列表消失 */
async function cancel(t: DownloadTask) {
  actionError.value = ''
  // 先记上：取消后后端可能还有在途的 progress 事件，不能让它把任务插回来
  markDownloadCanceled(t.fileName)
  try {
    await invoke('stop_download', { fileName: t.fileName })
  } catch (e) {
    const msg = String(e)
    if (!msg.includes('未在下载')) {
      actionError.value = msg
      return
    }
  }
  delete downloadStates[t.fileName]
  downloadTasks.value = downloadTasks.value.filter((x) => x.fileName !== t.fileName)
}

async function remove(fileName: string) {
  actionError.value = await clearDownloadTask(fileName)
}
</script>

<template>
  <!-- 右下角小圆圈：没有任务时默认不显示；有任务才出现并带角标，不带波纹 -->
  <button
    v-if="downloadTasks.length"
    class="dock-circle dock-circle--right"
    :class="{ active: taskPanelOpen }"
    title="下载任务"
    aria-label="下载任务"
    @click="taskPanelOpen = !taskPanelOpen"
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 4v10m0 0 4-4m-4 4-4-4M5 19h14" />
    </svg>
    <i v-if="activeDownloadCount" class="dock-badge">{{ activeDownloadCount }}</i>
  </button>

  <div v-if="taskPanelOpen" class="task-overlay" @click.self="taskPanelOpen = false">
    <aside class="task-drawer" role="dialog" aria-label="下载任务">
      <header class="task-head">
        <div>
          <div class="task-title">下载任务</div>
          <div class="meta" style="font-size: 12px">
            {{ activeDownloadCount }} 个进行中<template v-if="finishedCount">
              · {{ finishedCount }} 个已结束</template
            >
          </div>
        </div>
        <div style="display: flex; gap: 8px; align-items: center">
          <button
            v-if="finishedCount"
            class="btn-ghost"
            style="font-size: 12px"
            @click="clearFinishedDownloads"
          >
            清空已结束
          </button>
          <button class="btn-ghost" aria-label="关闭" @click="taskPanelOpen = false">✕</button>
        </div>
      </header>

      <p v-if="actionError" style="color: #ff7db0; font-size: 12px; padding: 0 4px 8px">
        {{ actionError }}
      </p>

      <div v-if="!downloadTasks.length" class="empty">还没有下载任务</div>
      <div v-else class="task-list">
        <div v-for="t in downloadTasks" :key="t.fileName" class="task-row">
          <div class="task-row-top">
            <div class="task-name" :title="t.name">{{ t.name }}</div>
            <span class="tag" :style="statusStyle(t.status, t.reused)">{{ statusText(t.status, t.reused) }}</span>
          </div>

          <div class="dl-progress">
            <div class="bar"><i :style="{ width: (t.percent || 0) + '%' }" /></div>
            <div class="pct">{{ (t.percent || 0).toFixed(0) }}%</div>
          </div>

          <div class="task-stats">
            <span>
              {{ fmtBytes(t.received) }}<template v-if="t.total"> / {{ fmtBytes(t.total) }}</template>
            </span>
            <span v-if="t.reused" style="color: var(--ink-dim)">本地已有客户端，未重新下载</span>
            <span v-if="t.status === 'downloading'" style="color: var(--cyan)">
              {{ fmtSpeed(t.speed) }}
            </span>
            <span v-if="t.status === 'downloading'">{{ t.threads }} 线程</span>
            <span v-if="t.status === 'error'" style="color: #ff8f8f">code={{ t.code }}</span>
          </div>

          <div class="task-actions">
            <button
              v-if="t.status === 'downloading'"
              class="btn-ghost"
              style="font-size: 12px; color: #ff8f8f"
              @click="cancel(t)"
            >
              取消
            </button>
            <button v-else class="btn-ghost" style="font-size: 12px" @click="remove(t.fileName)">
              移除
            </button>
          </div>
        </div>
      </div>
    </aside>
  </div>
</template>
