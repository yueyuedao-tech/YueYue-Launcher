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
  settings,
  store,
} from '../store'

const inst = computed(() => currentInstance())
/** 自定义背景图（个性化里选的）；没有则用内置 hero 图 */
const bgStyle = computed(() =>
  settings.bgImage
    ? {
        backgroundImage: `url("${settings.bgImage}")`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }
    : undefined,
)
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
    <div class="home-bg" aria-hidden="true" :style="bgStyle" />

    <header>
      <div class="brand">Yue<em>Yue</em> Launcher</div>
      <div class="brand-sub">YYL · Windows / Linux · Mindustry 游戏启动</div>

      <div class="launch-zone">
        <div class="version-pick">
          <label for="inst">当前游戏</label>
          <select id="inst" v-model="selectedInstanceId">
            <option v-for="i in instances" :key="i.id" :value="i.id">
              {{ i.name }}{{ i.running ? '（运行中）' : '' }}
            </option>
            <option v-if="!instances.length" value="" disabled>（无游戏，请先创建）</option>
          </select>
        </div>

        <button class="btn-grad" @click="launch">启 动</button>
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
