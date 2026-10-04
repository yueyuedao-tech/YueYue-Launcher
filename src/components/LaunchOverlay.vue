<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { LaunchExitPayload, LaunchLogPayload } from '../types'
import { closeLaunch, instances, store } from '../store'

const unlisteners: UnlistenFn[] = []
const logBox = ref<HTMLElement | null>(null)
const stopping = ref(false)

const instanceName = () => instances.value.find((i) => i.id === store.launch.instanceId)?.name ?? store.launch.instanceId

function append(text: string, cls = '') {
  store.launch.logs.push({ text, cls })
  requestAnimationFrame(() => {
    if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight
  })
}

onMounted(async () => {
  unlisteners.push(
    await listen<LaunchLogPayload>('launch-log', (e) => {
      if (e.payload.id !== store.launch.instanceId) return
      const prefix = e.payload.stream === 'err' ? '[err] ' : ''
      append(prefix + e.payload.line, e.payload.stream === 'err' ? 'warn' : '')
    }),
  )
  unlisteners.push(
    await listen<LaunchExitPayload>('launch-exit', (e) => {
      if (e.payload.id !== store.launch.instanceId) return
      store.launch.code = e.payload.code
      if (e.payload.code === 0) {
        store.launch.status = 'done'
        append('[进程] 已退出，code=0', 'ok')
      } else {
        store.launch.status = 'failed'
        append(`[进程] 异常退出，code=${e.payload.code}`, 'warn')
      }
    }),
  )
  append(`[启动器] 请求启动实例「${instanceName()}」…`)
})

onBeforeUnmount(() => {
  unlisteners.forEach((u) => u())
})

async function cancelOrClose() {
  if (store.launch.status === 'running') {
    stopping.value = true
    try {
      await invoke('stop_instance', { id: store.launch.instanceId })
      append('[启动器] 已停止进程', 'warn')
    } catch (e) {
      append(`[启动器] 停止失败: ${e}`, 'warn')
    } finally {
      stopping.value = false
      closeLaunch()
      store.launch.status = 'running'
      store.launch.logs = []
    }
  } else {
    closeLaunch()
    store.launch.status = 'running'
    store.launch.logs = []
  }
}
</script>

<template>
  <div class="launch-mask" role="dialog" aria-modal="true" aria-label="启动进度">
    <div class="launch-panel">
      <div class="launch-head" :class="{ done: store.launch.status !== 'running' }">
        <svg class="sparkle" viewBox="0 0 48 48" aria-hidden="true">
          <path
            d="M24 6l5 12.5L41.5 21 29 27.5 24 40l-5-12.5L6.5 21 19 18.5z"
            fill="none"
            stroke="url(#lg)"
            stroke-width="3"
            stroke-linejoin="round"
          />
          <defs>
            <linearGradient id="lg" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#ff7db0" />
              <stop offset="1" stop-color="#6ee7f9" />
            </linearGradient>
          </defs>
        </svg>
        <div>
          <h2>
            <template v-if="store.launch.status === 'running'">正在启动 {{ instanceName() }}</template>
            <template v-else-if="store.launch.status === 'done'">启动成功</template>
            <template v-else>启动失败</template>
          </h2>
          <div class="sub">
            <template v-if="store.launch.status === 'running'">真实进程运行中 · 日志实时滚动</template>
            <template v-else-if="store.launch.status === 'done'">游戏进程已正常退出（code=0）</template>
            <template v-else>进程退出码 {{ store.launch.code }}</template>
          </div>
        </div>
      </div>

      <div class="launch-bar" :class="{ pulse: store.launch.status === 'running' }">
        <i :style="store.launch.status === 'running' ? {} : { width: '100%' }" />
      </div>
      <div class="launch-meta">
        <span>{{ store.launch.status === 'running' ? '运行中 · 等待进程结束' : '已结束' }}</span>
        <span>{{ store.launch.logs.length }} 行日志</span>
      </div>

      <div ref="logBox" class="launch-log" aria-live="polite">
        <div v-for="(line, i) in store.launch.logs" :key="i" :class="line.cls">{{ line.text }}</div>
      </div>

      <div class="launch-actions">
        <button class="btn-ghost" :disabled="stopping" @click="cancelOrClose">
          {{ store.launch.status === 'running' ? (stopping ? '停止中…' : '停止并关闭') : '完成' }}
        </button>
      </div>
    </div>
  </div>
</template>
