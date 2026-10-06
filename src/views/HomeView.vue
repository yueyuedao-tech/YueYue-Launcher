<script setup lang="ts">
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { news } from '../data/mock'
import {
  autoCollapseLaunch,
  currentInstance,
  instances,
  loadInstances,
  markLaunchFailed,
  openLaunchLog,
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
    // 已在运行：直接打开它的日志（有归档就复用，别把之前跑的日志清掉）
    openLaunchLog(target.id)
    return
  }
  if (starting) return
  starting = true
  resetLaunch(target.id)
  try {
    await invoke('launch_instance', { id: target.id })
    // 启动完成 → 浮层自动收起，日志归到左下角圆圈（多开时各实例一份）
    autoCollapseLaunch(target.id)
  } catch (e) {
    markLaunchFailed(target.id, String(e))
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
