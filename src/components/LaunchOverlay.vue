<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  closeLaunch,
  launchRecords,
  loadInstances,
  markLaunchStopped,
  store,
} from '../store'

const logBox = ref<HTMLElement | null>(null)
const stopping = ref(false)

/** 浮层显示的就是 store.launch.instanceId 那条归档记录 */
const rec = computed(() => launchRecords[store.launch.instanceId])
const logs = computed(() => rec.value?.logs ?? [])
const status = computed(() => rec.value?.status ?? 'running')
const name = computed(() => rec.value?.name ?? store.launch.instanceId)

watch(
  () => logs.value.length,
  async () => {
    await nextTick()
    if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight
  },
  { immediate: true },
)

/** 只收起浮层：记录继续留在左下角圆圈里 */
function collapse() {
  closeLaunch()
}

async function stopAndCollapse() {
  stopping.value = true
  try {
    await invoke('stop_instance', { id: store.launch.instanceId })
    markLaunchStopped(store.launch.instanceId)
    await loadInstances()
  } catch (e) {
    // 停止失败也收起：背面还有左下角圆圈可以回看
    console.error('stop_instance failed', e)
  } finally {
    stopping.value = false
    closeLaunch()
  }
}
</script>

<template>
  <div class="launch-mask" role="dialog" aria-modal="true" aria-label="启动进度">
    <div class="launch-panel">
      <div class="launch-head" :class="{ done: status !== 'running' }">
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
            <template v-if="status === 'running'">正在启动 {{ name }}</template>
            <template v-else-if="status === 'done'">启动成功</template>
            <template v-else-if="status === 'stopped'">已停止</template>
            <template v-else>启动失败</template>
          </h2>
          <div class="sub">
            <template v-if="status === 'running'">真实进程运行中 · 日志实时滚动</template>
            <template v-else-if="status === 'done'">游戏进程已正常退出（code=0）</template>
            <template v-else-if="status === 'stopped'">进程已被启动器停止</template>
            <template v-else>进程退出码 {{ rec?.code }}</template>
          </div>
        </div>
      </div>

      <div class="launch-bar" :class="{ pulse: status === 'running' }">
        <i :style="status === 'running' ? {} : { width: '100%' }" />
      </div>
      <div class="launch-meta">
        <span>{{ status === 'running' ? '运行中 · 等待进程结束' : '已结束' }}</span>
        <span>{{ logs.length }} 行日志</span>
      </div>

      <div ref="logBox" class="launch-log" aria-live="polite">
        <div v-for="(line, i) in logs" :key="i" :class="line.cls">{{ line.text }}</div>
      </div>

      <div class="launch-actions">
        <button class="btn-ghost" @click="collapse">收起到左下角</button>
        <button
          v-if="status === 'running'"
          class="btn-ghost"
          style="color: #ff8f8f"
          :disabled="stopping"
          @click="stopAndCollapse"
        >
          {{ stopping ? '停止中…' : '停止并收起' }}
        </button>
        <button v-else class="btn-grad" @click="collapse">完成</button>
      </div>
    </div>
  </div>
</template>
