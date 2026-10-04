<script setup lang="ts">
import { settings } from '../store'
</script>

<template>
  <section class="page">
    <h1 class="page-title">设置</h1>
    <p class="page-sub">偏好保存在本地（localStorage）；具体启动参数以各实例的 launch.config.json 为准</p>

    <div class="settings-grid">
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
      </div>

      <div class="set-group">
        <h3>启动</h3>
        <div class="set-row">
          <div>
            <div class="label">新实例默认内存</div>
            <div class="hint">创建实例时的初始值，创建后可在实例里单独改</div>
          </div>
          <div class="ctrl">
            <input v-model.number="settings.memory" type="range" min="1024" max="16384" step="512" />
            <span class="val">{{ settings.memory }} MB</span>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
