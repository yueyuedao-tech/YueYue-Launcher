<script setup lang="ts">
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { marked } from 'marked'
import {
  autoCollapseLaunch,
  currentInstance,
  effectiveInfoBar,
  instances,
  loadInstances,
  markLaunchFailed,
  openLaunchLog,
  resetLaunch,
  selectedInstanceId,
  settings,
  store,
} from '../store'

/**
 * 每日信息：一行一条，行首 `**日期 · 标签**` 之后是标题。
 * 拆成 date / title 两块，仍用原本的 .news-card 结构渲染，样式不变。
 */
const newsCards = computed(() =>
  effectiveInfoBar.value
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const m = line.match(/^\*\*(.+?)\*\*\s*(.*)$/)
      return m ? { date: m[1], title: m[2] } : { date: '', title: line }
    }),
)

/** 标题里的行内 Markdown（加粗/链接/代码）；先转义 `<` 再渲染，避免注入 */
function inlineMd(text: string): string {
  return marked.parseInline(text.replace(/</g, '&lt;'), { async: false }) as string
}

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
      <article v-for="(n, i) in newsCards" :key="i" class="news-card">
        <div v-if="n.date" class="date">{{ n.date }}</div>
        <div class="title" v-html="inlineMd(n.title)" />
      </article>
    </section>
  </div>
</template>
