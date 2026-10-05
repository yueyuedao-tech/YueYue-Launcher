<script setup lang="ts">
import { computed } from 'vue'
import { ACCENT_PAIRS, settings, settingsNavPos } from '../store'

/** 设置页分类导航位置：读取时给默认值（跟随主导航），写入时才落盘为独立设置 */
const settingsNav = computed<'left' | 'top'>({
  get: () => settingsNavPos.value,
  set: (v) => {
    settings.settingsNavPosition = v
  },
})
</script>

<template>
  <div class="settings-grid">
    <div class="set-group">
      <h3>导航位置</h3>
      <div class="set-row">
        <div>
          <div class="label">主导航栏位置</div>
        </div>
        <div class="ctrl" style="gap: 8px">
          <label class="btn-ghost" :style="settings.navPosition === 'left' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settings.navPosition" type="radio" name="main-nav-pos" value="left" style="margin-right: 6px" />左侧
          </label>
          <label class="btn-ghost" :style="settings.navPosition === 'top' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settings.navPosition" type="radio" name="main-nav-pos" value="top" style="margin-right: 6px" />顶部
          </label>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">设置页导航位置</div>
        </div>
        <div class="ctrl" style="gap: 8px">
          <label class="btn-ghost" :style="settingsNav === 'left' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settingsNav" type="radio" name="settings-nav-pos" value="left" style="margin-right: 6px" />左侧
          </label>
          <label class="btn-ghost" :style="settingsNav === 'top' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settingsNav" type="radio" name="settings-nav-pos" value="top" style="margin-right: 6px" />顶部
          </label>
        </div>
      </div>
    </div>

    <div class="set-group">
      <h3>外观</h3>
      <div class="set-row">
        <div>
          <div class="label">主题</div>
        </div>
        <div class="ctrl">
          <select v-model="settings.theme" class="field">
            <option>暗色</option>
            <option>跟随系统</option>
          </select>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">强调色</div>
        </div>
        <div class="ctrl">
          <select v-model="settings.accent" class="field">
            <option v-for="(_, k) in ACCENT_PAIRS" :key="k" :value="k">{{ k }}</option>
          </select>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">主页背景遮罩强度</div>
        </div>
        <div class="ctrl">
          <input v-model.number="settings.bgShade" type="range" min="0" max="100" step="2" />
          <span class="val">{{ settings.bgShade }}</span>
        </div>
      </div>
    </div>

    <div class="set-group">
      <h3>窗口</h3>
      <div class="set-row">
        <div>
          <div class="label">窗口置顶</div>
        </div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="settings.alwaysOnTop" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">关闭按钮行为</div>
        </div>
        <div class="ctrl">
          <select v-model="settings.closeBehavior" class="field">
            <option value="exit">直接退出</option>
            <option value="minimize">最小化到托盘</option>
          </select>
        </div>
      </div>
    </div>
  </div>
</template>
