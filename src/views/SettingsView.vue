<script setup lang="ts">
import { ref } from 'vue'
import { settings } from '../store'

type Tab = 'download' | 'mirror' | 'github' | 'launch'
const active = ref<Tab>('download')
const tabs: { id: Tab; label: string }[] = [
  { id: 'download', label: '下载' },
  { id: 'mirror', label: '镜像与网络' },
  { id: 'github', label: 'GitHub' },
  { id: 'launch', label: '启动' },
]
</script>

<template>
  <section class="page">
    <h1 class="page-title">设置</h1>
    <p class="page-sub">分区配置；个性化（外观/窗口/导航）已移至「个性化」页</p>

    <!-- 顶部居中分区 tab -->
    <div class="section-tabs" role="tablist">
      <button
        v-for="t in tabs"
        :key="t.id"
        class="section-tab"
        :class="{ active: active === t.id }"
        role="tab"
        @click="active = t.id"
      >
        {{ t.label }}
      </button>
    </div>

    <div class="settings-grid" style="margin-top: 16px">
      <div v-show="active === 'download'" class="set-group">
        <h3>下载</h3>
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

      <!-- 镜像与网络：统一优先级 = 镜像 → 代理 → 直连 -->
      <div v-show="active === 'mirror'" class="set-group">
        <h3>镜像与网络</h3>
        <div class="set-row">
          <div>
            <div class="label">下载代理</div>
            <div class="hint">全局代理，示例 127.0.0.1:7897；镜像为空时生效</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 320px">
            <input v-model="settings.proxy" class="field" style="width: 100%" placeholder="127.0.0.1:7897" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">GitHub 加速前缀</div>
            <div class="hint">拼在 github.com 前（jar/Atom/版本索引）</div>
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
            <div class="label">工坊镜像</div>
            <div class="hint">拼在 steamcommunity.com 前（搜索用）；留空走代理</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="settings.workshopMirror" class="field" style="width: 100%" placeholder="https://your-steam-mirror/" />
          </div>
        </div>
      </div>

      <div v-show="active === 'github'" class="set-group">
        <h3>GitHub</h3>
        <div class="set-row">
          <div>
            <div class="label">新版本小红点</div>
            <div class="hint">启动时静默检查 GitHub 最新版本，失败不提示；加速前缀在「镜像与网络」配置</div>
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

      <div v-show="active === 'launch'" class="set-group">
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
