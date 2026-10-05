<script setup lang="ts">
import { ACCENT_PAIRS, settings } from '../store'
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

      <div class="set-group">
        <h3>下载</h3>
        <div class="set-row">
          <div>
            <div class="label">下载代理</div>
            <div class="hint">留空 = 直连；示例 127.0.0.1:7897（每次下载生效）</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 320px">
            <input v-model="settings.proxy" class="field" style="width: 100%" placeholder="127.0.0.1:7897" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">下载目录</div>
            <div class="hint">留空 = 默认（%APPDATA%\StarlightLauncher\downloads）</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="settings.downloadDir" class="field" style="width: 100%" placeholder="D:\Mindustry下载" />
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>GitHub</h3>
        <div class="set-row">
          <div>
            <div class="label">加速前缀（镜像）</div>
            <div class="hint">拼在 github.com 之前，用于版本索引与下载；留空 = 直连</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input
              v-model="settings.githubPrefix"
              class="field"
              style="width: 100%"
              list="gh-prefix-hints"
              placeholder="https://example-ghproxy.com/"
            />
            <datalist id="gh-prefix-hints">
              <option value="https://ghfast.top/" />
              <option value="https://gh-proxy.com/" />
              <option value="https://mirror.ghproxy.com/" />
            </datalist>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">新版本小红点</div>
            <div class="hint">启动时静默检查 GitHub 最新版本，失败不提示</div>
          </div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="settings.updateCheck" type="checkbox" />
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
