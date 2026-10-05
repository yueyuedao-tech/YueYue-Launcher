<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  instances,
  loadInstances,
  selectedInstanceId,
  settings,
  settingsNavPos,
  settingsTab,
  type SettingsTab,
} from '../store'
import PersonalControls from './PersonalControls.vue'

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'download', label: '下载' },
  { id: 'mirror', label: '镜像与网络' },
  { id: 'launch', label: '启动' },
  { id: 'game', label: '游戏' },
  { id: 'personal', label: '个性化' },
]

function pick(id: SettingsTab) {
  settingsTab.value = id
}

/** 存档操作针对当前选中的游戏（没有就取列表第一个） */
const targetId = computed(() => selectedInstanceId.value || instances.value[0]?.id || '')
const targetName = computed(
  () => instances.value.find((i) => i.id === targetId.value)?.name ?? '',
)
const saves = ref<string[]>([])
const newSaveName = ref('')
const saveError = ref('')

async function refreshSaves() {
  saveError.value = ''
  if (!targetId.value) {
    saves.value = []
    return
  }
  try {
    saves.value = (await invoke('list_saves', { id: targetId.value })) as string[]
  } catch (e) {
    saveError.value = String(e)
    saves.value = []
  }
}

async function createSave() {
  const name = newSaveName.value.trim()
  if (!name || !targetId.value) return
  try {
    saves.value = (await invoke('create_save', { id: targetId.value, name })) as string[]
    newSaveName.value = ''
    saveError.value = ''
  } catch (e) {
    saveError.value = String(e)
  }
}

onMounted(async () => {
  await loadInstances()
  await refreshSaves()
})
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
            <div class="label">启用存档隔离</div>
          </div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="settings.saveIsolation" type="checkbox" />
              <span class="track" />
              <span class="thumb" />
            </label>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">新游戏默认内存</div>
          </div>
          <div class="ctrl">
            <input v-model.number="settings.memory" type="range" min="1024" max="16384" step="512" />
            <span class="val">{{ settings.memory }} MB</span>
          </div>
        </div>
      </div>

      <!-- 游戏：删除默认方式 + 存档管理 -->
      <div v-show="settingsTab === 'game'" class="set-group">
        <h3>删除</h3>
        <div class="set-row">
          <div>
            <div class="label">默认删除方式</div>
          </div>
          <div class="ctrl" style="gap: 8px">
            <label class="btn-ghost" :style="settings.defaultDelete === 'keep' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
              <input v-model="settings.defaultDelete" type="radio" name="del-mode" value="keep" style="margin-right: 6px" />删本体留存档
            </label>
            <label class="btn-ghost" :style="settings.defaultDelete === 'purge' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
              <input v-model="settings.defaultDelete" type="radio" name="del-mode" value="purge" style="margin-right: 6px" />彻底删除
            </label>
          </div>
        </div>
      </div>

      <div v-show="settingsTab === 'game'" class="set-group">
        <h3>存档{{ targetName ? ` · ${targetName}` : '' }}</h3>
        <div class="set-row">
          <div>
            <div class="label">新开存档</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px; gap: 8px">
            <input v-model="newSaveName" class="field" style="width: 100%" placeholder="存档名称" :disabled="!targetId" @keyup.enter="createSave" />
            <button class="btn-ghost" style="flex: none" :disabled="!targetId || !newSaveName.trim()" @click="createSave">新建</button>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">扫描存档</div>
          </div>
          <div class="ctrl">
            <button class="btn-ghost" :disabled="!targetId" @click="refreshSaves">扫描</button>
          </div>
        </div>
        <div v-if="saveError" class="set-row">
          <div style="color: #ff7db0; font-size: 13px">{{ saveError }}</div>
        </div>
        <div v-for="s in saves" :key="s" class="set-row">
          <div>
            <div class="label">{{ s }}</div>
          </div>
          <div class="ctrl">
            <span class="tag" style="background: rgba(255,255,255,.07); color: var(--ink-dim)">
              {{ settings.saveIsolation ? '隔离中' : '共享' }}
            </span>
          </div>
        </div>
        <div v-if="!saves.length && !saveError" class="set-row">
          <div class="label" style="color: var(--ink-dim)">暂无存档</div>
        </div>
      </div>

      <!-- 个性化（原独立页已归档至此，数据同源） -->
      <PersonalControls v-show="settingsTab === 'personal'" />
    </div>
  </section>
</template>
