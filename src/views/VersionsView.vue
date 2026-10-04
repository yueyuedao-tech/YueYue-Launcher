<script setup lang="ts">
import { computed, ref } from 'vue'
import { versions } from '../data/mock'
import { settings, store } from '../store'

const keyword = ref('')

const filtered = computed(() =>
  versions.filter((v) => {
    if (!settings.showSnapshots && v.type === '快照版') return false
    return v.name.toLowerCase().includes(keyword.value.trim().toLowerCase())
  }),
)

function tagClass(type: string) {
  if (type === '正式版') return 'tag stable'
  if (type === '整合包') return 'tag mod'
  return 'tag'
}
</script>

<template>
  <section class="page">
    <h1 class="page-title">版本列表</h1>
    <p class="page-sub">已安装的版本与整合包，共 {{ filtered.length }} 个</p>

    <div class="toolbar">
      <input v-model="keyword" class="field" type="search" placeholder="按名称过滤…" aria-label="过滤版本" />
      <span class="tag">快照 {{ settings.showSnapshots ? '显示' : '隐藏' }}</span>
    </div>

    <div v-if="filtered.length" class="list">
      <div
        v-for="v in filtered"
        :key="v.id"
        class="row-card"
        :style="store.selectedVersionId === v.id ? { borderColor: 'rgba(255,125,176,.55)' } : undefined"
      >
        <div class="row-icon" :class="{ cy: v.type === '正式版' }">
          {{ v.type === '整合包' ? '🎁' : v.type === '快照版' ? '🧪' : '📦' }}
        </div>
        <div class="row-main">
          <div class="name">{{ v.name }}</div>
          <div class="meta">上次启动：{{ v.lastPlayed }} · 占用 {{ v.size }}</div>
        </div>
        <div class="row-side">
          <span :class="tagClass(v.type)">{{ v.type }}</span>
          <button class="btn-ghost" @click="store.selectedVersionId = v.id">
            {{ store.selectedVersionId === v.id ? '已选择' : '设为当前' }}
          </button>
        </div>
      </div>
    </div>
    <div v-else class="empty">没有匹配的版本，换个关键词试试</div>
  </section>
</template>
