<script setup lang="ts">
import { settings } from '../store'
</script>

<template>
  <section class="page">
    <h1 class="page-title">设置</h1>
    <p class="page-sub">偏好设置会保存在本地（localStorage），重启后保持</p>

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
        <div class="set-row">
          <div>
            <div class="label">在下载页显示快照版本</div>
            <div class="hint">关闭后版本列表隐藏快照版</div>
          </div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="settings.showSnapshots" type="checkbox" />
              <span class="track" />
              <span class="thumb" />
            </label>
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>启动</h3>
        <div class="set-row">
          <div>
            <div class="label">游戏内存</div>
            <div class="hint">模拟启动日志中会展示该分配值</div>
          </div>
          <div class="ctrl">
            <input v-model.number="settings.memory" type="range" min="1024" max="16384" step="512" />
            <span class="val">{{ settings.memory }} MB</span>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">Java 环境</div>
            <div class="hint">真实检测将在后续版本提供</div>
          </div>
          <div class="ctrl">
            <select v-model="settings.javaPath" class="field">
              <option>自动检测</option>
              <option>Java 21</option>
              <option>Java 17</option>
            </select>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">窗口分辨率</div>
            <div class="hint">模拟启动参数</div>
          </div>
          <div class="ctrl">
            <input v-model.number="settings.width" class="field" type="number" min="800" style="width: 90px" />
            <span style="color: var(--ink-dim)">×</span>
            <input v-model.number="settings.height" class="field" type="number" min="600" style="width: 90px" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">游戏启动后保持启动器开启</div>
            <div class="hint">关闭时启动器将在拉起游戏后退出</div>
          </div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="settings.keepOpen" type="checkbox" />
              <span class="track" />
              <span class="thumb" />
            </label>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
