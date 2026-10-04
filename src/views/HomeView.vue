<script setup lang="ts">
import { computed, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { news } from '../data/mock'
import {
  currentInstance,
  instances,
  loadInstances,
  resetLaunch,
  selectedInstanceId,
  store,
} from '../store'

const inst = computed(() => currentInstance())
let starting = false

async function launch() {
  const target = inst.value
  if (!target) {
    store.view = 'instances'
    return
  }
  if (target.running) {
    // 已在运行：直接打开浮层看日志（不重复拉起）
    resetLaunch(target.id)
    return
  }
  if (starting) return
  starting = true
  resetLaunch(target.id)
  await nextTick() // 等浮层挂载并注册事件监听
  try {
    await invoke('launch_instance', { id: target.id })
  } catch (e) {
    store.launch.logs.push({ text: `[错误] ${e}`, cls: 'warn' })
    store.launch.status = 'failed'
  } finally {
    starting = false
    loadInstances()
  }
}
</script>

<template>
  <div class="home">
    <div class="home-bg" aria-hidden="true" />

    <header>
      <div class="brand">星<em>启</em>启动器</div>
      <div class="brand-sub">STARLIGHT LAUNCHER · Windows / Linux · Mindustry 实例启动</div>

      <div class="launch-zone">
        <div class="version-pick">
          <label for="inst">当前实例</label>
          <select id="inst" v-model="selectedInstanceId">
            <option v-for="i in instances" :key="i.id" :value="i.id">
              {{ i.name }}{{ i.running ? '（运行中）' : '' }}
            </option>
            <option v-if="!instances.length" value="" disabled>（无实例，请先创建）</option>
          </select>
        </div>

        <button class="btn-grad" @click="launch">启 动</button>
        <div style="color: var(--ink-dim); font-size: 12px">
          <template v-if="inst">
            {{ inst.jarPath }} · 数据隔离 {{ inst.isolate ? '开' : '关' }} · Java {{ inst.javaPath }}
          </template>
          <template v-else>尚无实例——点击后前往实例管理创建</template>
        </div>
      </div>
    </header>

    <section class="news-row" aria-label="公告">
      <article v-for="n in news" :key="n.title" class="news-card">
        <div class="date">{{ n.date }} · {{ n.tag }}</div>
        <div class="title">{{ n.title }}</div>
      </article>
    </section>
  </div>
</template>
