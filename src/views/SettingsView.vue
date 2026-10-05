<script setup lang="ts">
import { settings, settingsNavPos, settingsTab, type SettingsTab } from '../store'
import PersonalControls from './PersonalControls.vue'

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'download', label: '下载' },
  { id: 'mirror', label: '镜像与网络' },
  { id: 'launch', label: '启动' },
  { id: 'personal', label: '个性化' },
]

function pick(id: SettingsTab) {
  settingsTab.value = id
}
</script>

<template>
  <section
    class="page settings-page"
    :class="settingsNavPos === 'top' ? 'settings-page--topnav' : 'settings-page--sidenav'"
  >
    <!-- 设置专属分类导航栏：默认跟随主导航位置 -->
    <nav
      class="settings-nav"
      :class="{ 'settings-nav--top': settingsNavPos === 'top' }"
      aria-label="设置分类"
    >
      <h1 class="settings-nav-title">设置</h1>
      <button
        v-for="t in tabs"
        :key="t.id"
        class="settings-nav-btn"
        :class="{ active: settingsTab === t.id }"
        role="tab"
        :aria-selected="settingsTab === t.id"
        @click="pick(t.id)"
      >
        {{ t.label }}
      </button>
    </nav>

    <div class="settings-body">
      <div v-show="settingsTab === 'download'" class="set-group">
        <h3>下载</h3>
        <div class="set-row">
          <div>
            <div class="label">下载目录</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="settings.downloadDir" class="field" style="width: 100%" placeholder="D:\Mindustry下载" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">中心化服务器</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="settings.centralServer" class="field" style="width: 100%" placeholder="http://127.0.0.1:8787" />
          </div>
        </div>
      </div>

      <!-- 镜像与网络：地址用镜像前缀、连接用代理，可叠加 -->
      <div v-show="settingsTab === 'mirror'" class="set-group">
        <h3>镜像与网络</h3>
        <div class="set-row">
          <div>
            <div class="label">下载代理</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 320px">
            <input v-model="settings.proxy" class="field" style="width: 100%" placeholder="127.0.0.1:7897" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">GitHub 加速前缀</div>
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
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input v-model="settings.workshopMirror" class="field" style="width: 100%" placeholder="https://your-steam-mirror/" />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">新版本小红点</div>
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

      <div v-show="settingsTab === 'launch'" class="set-group">
        <h3>启动</h3>
        <div class="set-row">
          <div>
            <div class="label">新实例默认内存</div>
          </div>
          <div class="ctrl">
            <input v-model.number="settings.memory" type="range" min="1024" max="16384" step="512" />
            <span class="val">{{ settings.memory }} MB</span>
          </div>
        </div>
      </div>

      <!-- 个性化（原独立页已归档至此，数据同源） -->
      <PersonalControls v-show="settingsTab === 'personal'" />
    </div>
  </section>
</template>
