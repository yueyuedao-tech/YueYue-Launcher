<script setup lang="ts">
import { onBeforeUnmount, reactive } from 'vue'
import { downloadables } from '../data/mock'

interface DlState {
  progress: number
  timer?: ReturnType<typeof setInterval>
}

const state = reactive<Record<string, DlState>>(
  Object.fromEntries(downloadables.map((d) => [d.id, { progress: 0 }])),
)

function start(id: string) {
  const s = state[id]
  if (s.timer) return
  s.timer = setInterval(() => {
    s.progress = Math.min(100, s.progress + 4)
    if (s.progress >= 100) {
      clearInterval(s.timer!)
      s.timer = undefined
    }
  }, 160)
}

function reset(id: string) {
  const s = state[id]
  if (s.timer) clearInterval(s.timer)
  s.timer = undefined
  s.progress = 0
}

function label(id: string) {
  const s = state[id]
  if (s.timer) return '下载中…'
  if (s.progress >= 100) return '重新下载'
  if (s.progress > 0) return '继续'
  return '下载'
}

onBeforeUnmount(() => {
  for (const s of Object.values(state)) if (s.timer) clearInterval(s.timer)
})
</script>

<template>
  <section class="page">
    <h1 class="page-title">下载中心</h1>
    <p class="page-sub">可下载的版本与整合包（V1 为模拟下载进度，不产生真实流量）</p>

    <div class="list">
      <div v-for="d in downloadables" :key="d.id" class="row-card">
        <div class="row-icon cy">{{ d.type === '整合包' ? '🧩' : d.type === '快照版' ? '🔭' : '🌐' }}</div>
        <div class="row-main">
          <div class="name">{{ d.name }}</div>
          <div class="meta">{{ d.desc }} · {{ d.size }}</div>
        </div>
        <div class="row-side">
          <div v-if="state[d.id].progress > 0" class="dl-progress">
            <div class="bar"><i :style="{ width: state[d.id].progress + '%' }" /></div>
            <div class="pct">{{ state[d.id].progress }}%</div>
          </div>
          <span :class="d.type === '正式版' ? 'tag stable' : d.type === '整合包' ? 'tag mod' : 'tag'">{{ d.type }}</span>
          <button class="btn-ghost" @click="state[d.id].progress >= 100 ? reset(d.id) : start(d.id)">
            {{ label(d.id) }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
