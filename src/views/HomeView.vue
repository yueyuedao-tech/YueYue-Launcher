<script setup lang="ts">
import { news, versions } from '../data/mock'
import { selectedVersionName, store } from '../store'

function launch() {
  store.launch.open = true
  store.launch.progress = 0
  store.launch.status = 'running'
  store.launch.logs = []
}
</script>

<template>
  <div class="home">
    <div class="home-bg" aria-hidden="true" />

    <header>
      <div class="brand">星<em>启</em>启动器</div>
      <div class="brand-sub">STARLIGHT LAUNCHER · Windows / Linux · 模拟启动演示版</div>

      <div class="launch-zone">
        <div class="version-pick">
          <label for="ver">当前版本</label>
          <select id="ver" v-model="store.selectedVersionId">
            <option v-for="v in versions" :key="v.id" :value="v.id">
              {{ v.name }}（{{ v.type }}）
            </option>
          </select>
        </div>

        <button class="btn-grad" @click="launch">启 动</button>
        <div style="color: var(--ink-dim); font-size: 12px">
          已选：{{ selectedVersionName() }} · 点击启动将运行模拟流程
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
