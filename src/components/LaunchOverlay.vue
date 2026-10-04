<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { selectedVersionName, settings, store } from '../store'

const PHASES: { at: number; text: string; cls: string }[] = [
  { at: 4, text: '[启动器] 校验游戏文件完整性……', cls: '' },
  { at: 12, text: '[启动器] 校验通过，共 1,024 个文件', cls: 'ok' },
  { at: 22, text: '[版本] 解析版本 JSON：' + selectedVersionName(), cls: '' },
  { at: 34, text: '[资源] 下载资源索引 assets index……', cls: '' },
  { at: 48, text: '[资源] 资源下载完成（模拟）', cls: 'ok' },
  { at: 58, text: '[Java] 检测运行环境 Java 21 x64', cls: '' },
  { at: 68, text: '[Java] 分配内存 ' + settings.memory + ' MB', cls: '' },
  { at: 78, text: '[启动] 拉起游戏进程 pid=23333（模拟）', cls: 'warn' },
  { at: 90, text: '[游戏] 渲染引擎初始化中……', cls: '' },
  { at: 100, text: '[游戏] 已进入主菜单，祝你玩得开心！', cls: 'ok' },
]

const fired = ref(new Set<number>())
const cancelText = ref('取消')

let timer: ReturnType<typeof setInterval> | undefined

function tick() {
  const launch = store.launch
  launch.progress = Math.min(100, launch.progress + 2)
  for (let i = 0; i < PHASES.length; i++) {
    const phase = PHASES[i]
    if (launch.progress >= phase.at && !fired.value.has(i)) {
      fired.value.add(i)
      launch.logs.push({ text: phase.text, cls: phase.cls })
    }
  }
  if (launch.progress >= 100) {
    launch.status = 'done'
    clearInterval(timer)
  }
}

function cancel() {
  clearInterval(timer)
  store.launch.open = false
}

function close() {
  clearInterval(timer)
  store.launch.open = false
  store.launch.progress = 0
  store.launch.status = 'running'
  store.launch.logs = []
}

onMounted(() => {
  timer = setInterval(tick, 130)
})

onBeforeUnmount(() => clearInterval(timer))
</script>

<template>
  <div class="launch-mask" role="dialog" aria-modal="true" aria-label="启动进度">
    <div class="launch-panel">
      <div class="launch-head" :class="{ done: store.launch.status === 'done' }">
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
          <h2>{{ store.launch.status === 'done' ? '启动成功' : '正在启动 ' + selectedVersionName() }}</h2>
          <div class="sub">
            {{ store.launch.status === 'done' ? '模拟启动完成（V1 不会真实运行游戏）' : '模拟启动流程，请稍候' }}
          </div>
        </div>
      </div>

      <div class="launch-bar"><i :style="{ width: store.launch.progress + '%' }" /></div>
      <div class="launch-meta">
        <span>进度 {{ store.launch.progress }}%</span>
        <span>{{ store.launch.status === 'done' ? '完成' : '执行中…' }}</span>
      </div>

      <div class="launch-log" aria-live="polite">
        <div v-for="(line, i) in store.launch.logs" :key="i" :class="line.cls">{{ line.text }}</div>
      </div>

      <div class="launch-actions">
        <button v-if="store.launch.status === 'running'" class="btn-ghost" @click="cancel">
          {{ cancelText }}
        </button>
        <button v-else class="btn-grad" style="font-size: 14px; padding: 10px 30px" @click="close">
          完成
        </button>
      </div>
    </div>
  </div>
</template>
