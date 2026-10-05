<script setup lang="ts">
import { ACCENT_PAIRS, settings } from '../store'
</script>

<template>
  <section class="page">
    <h1 class="page-title">个性化</h1>
    <p class="page-sub">外观、窗口与导航位置——即时生效并持久化</p>

    <div class="settings-grid">
      <div class="set-group">
        <h3>导航位置</h3>
        <div class="set-row">
          <div>
            <div class="label">主导航栏位置</div>
            <div class="hint">左侧图标栏 ↔ 顶部居中栏，一键互换</div>
          </div>
          <div class="ctrl" style="gap: 8px">
            <label class="btn-ghost" :style="settings.navPosition === 'left' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
              <input v-model="settings.navPosition" type="radio" value="left" style="margin-right: 6px" />左侧
            </label>
            <label class="btn-ghost" :style="settings.navPosition === 'top' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
              <input v-model="settings.navPosition" type="radio" value="top" style="margin-right: 6px" />顶部
            </label>
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>外观</h3>
        <div class="set-row">
          <div>
            <div class="label">主题</div>
            <div class="hint">V1 仅内置暗色主题</div>
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
            <div class="hint">渐变按钮/高亮/标签整体换色</div>
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
            <div class="hint">越高背景越暗（文字越清晰），默认 96</div>
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
            <div class="hint">启动器始终浮在最上层</div>
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
            <div class="hint">最小化到托盘时可用托盘菜单恢复/退出</div>
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
  </section>
</template>
