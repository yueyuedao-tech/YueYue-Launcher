<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { open as openExternal } from '@tauri-apps/plugin-shell'
import {
  effectiveProxyLabel,
  infoBarError,
  infoBarLoading,
  instances,
  loadCachedMirrors,
  loadInstances,
  loadSystemProxy,
  selectedInstanceId,
  settings,
  settingsNavPos,
  settingsTab,
  type SettingsTab,
} from '../store'
import PersonalControls from './PersonalControls.vue'

/** 中心化服务器列表：可增删，按顺序回退 */
function addServer() {
  settings.centralServers.push('')
}

function removeServer(i: number) {
  settings.centralServers.splice(i, 1)
}

/** 打开当前生效的下载目录（没设自定义目录时打开默认目录） */
async function openDownloadDir() {
  saveError.value = ''
  try {
    const dir = (await invoke('downloads_dir', {
      downloadDir: settings.downloadDir,
    })) as string
    await openExternal(dir)
  } catch (e) {
    saveError.value = String(e)
  }
}

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'download', label: '下载' },
  { id: 'mirror', label: '镜像与网络' },
  { id: 'launch', label: '启动' },
  { id: 'game', label: '游戏' },
  { id: 'personal', label: '个性化' },
  { id: 'dev', label: '开发者' },
  { id: 'about', label: '关于' },
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
const saveError = ref('')
/** 「关于」页显示的版本号：开机问 Tauri 要，避免页面里写死后发新版不更新 */
const appVersion = ref('')

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

onMounted(async () => {
  await loadInstances()
  await refreshSaves()
  void loadSystemProxy()
  void loadCachedMirrors()
  // 版本号直接问 Tauri 要（tauri.conf.json 里的 version），
  // 不再在页面里写死 —— 以前写死 0.1.0，发新版后「关于」还显示旧版本
  void getVersion()
    .then((v) => {
      appVersion.value = v
    })
    .catch(() => {
      appVersion.value = ''
    })
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
            <button class="btn-ghost" style="flex: none; font-size: 12px" @click="openDownloadDir">
              打开下载目录
            </button>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">下载线程数</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <select v-model.number="settings.downloadThreads" class="field" style="width: 100%">
              <option :value="1">1（单连接）</option>
              <option :value="2">2</option>
              <option :value="4">4</option>
              <option :value="8">8</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 镜像与网络：代理来源可选，镜像地址由中心服务器下发 -->
      <div v-show="settingsTab === 'mirror'" class="set-group">
        <h3>镜像与网络</h3>
        <div class="set-row">
          <div>
            <div class="label">下载代理</div>
            <div class="hint" style="font-size: 12px">{{ effectiveProxyLabel }}</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 460px; gap: 10px; flex-wrap: wrap">
            <select v-model="settings.proxyMode" class="field" style="flex: none">
              <option value="system">使用系统代理</option>
              <option value="custom">自定义</option>
            </select>
            <input
              v-if="settings.proxyMode === 'custom'"
              v-model="settings.proxy"
              class="field"
              style="flex: 1; min-width: 180px"
              placeholder="127.0.0.1:7897"
            />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">中心化服务器</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 520px; flex-direction: column; align-items: stretch; gap: 8px">
            <div
              v-for="(s, i) in settings.centralServers"
              :key="'srv-' + i"
              style="display: flex; gap: 8px; align-items: center"
            >
              <span class="srv-index">{{ i + 1 }}</span>
              <input
                v-model="settings.centralServers[i]"
                class="field"
                style="flex: 1; min-width: 160px"
                placeholder="http://127.0.0.1:8787"
              />
              <button class="btn-ghost" style="flex: none; font-size: 12px" @click="removeServer(i)">
                删除
              </button>
            </div>
            <div v-if="!settings.centralServers.length" class="meta" style="font-size: 12px">
              未配置（使用内置索引）
            </div>
            <button class="btn-ghost" style="align-self: flex-start; font-size: 12px" @click="addServer">
              + 添加服务器
            </button>
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
            <div class="label">每日信息（首页公告）</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 560px; gap: 10px; flex-wrap: wrap">
            <label class="switch" style="flex: none">
              <input v-model="settings.infoBarEnabled" type="checkbox" />
              <span class="track" />
              <span class="thumb" />
            </label>
            <input
              v-model="settings.infoBarUrl"
              class="field"
              style="flex: 1; min-width: 200px"
              :disabled="!settings.infoBarEnabled"
              placeholder="https://example.com/news.md"
            />
            <span v-if="infoBarLoading" class="meta" style="font-size: 12px; flex-basis: 100%">读取中…</span>
            <span
              v-else-if="infoBarError"
              class="meta"
              style="font-size: 12px; flex-basis: 100%; color: #ff8f8f"
            >
              {{ infoBarError }}
            </span>
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
        <h3>新建游戏默认值</h3>
        <div class="set-row">
          <div>
            <div class="label">JVM 参数</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input
              v-model="settings.defaultJvmArgs"
              class="field"
              style="width: 100%"
              placeholder="留空 = 不加，如 -XX:+UseG1GC"
            />
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">游戏参数</div>
          </div>
          <div class="ctrl" style="flex: 1; max-width: 420px">
            <input
              v-model="settings.defaultGameArgs"
              class="field"
              style="width: 100%"
              placeholder="留空 = 不加"
            />
          </div>
        </div>
      </div>

      <div v-show="settingsTab === 'game'" class="set-group">
        <h3>存档{{ targetName ? ` · ${targetName}` : '' }}</h3>
        <div class="set-row">
          <div>
            <div class="label">扫描存档</div>
            <div class="hint" style="font-size: 12px">
              {{ settings.saveIsolation ? '隔离实例：读 <实例>/data/Mindustry/saves' : '共享实例：读系统 AppData/Mindustry/saves' }}
            </div>
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

      <!-- 开发者：没写完的功能默认藏起来，在这里放开预览 -->
      <div v-show="settingsTab === 'dev'" class="set-group">
        <h3>开发者</h3>
        <div class="set-row">
          <div>
            <div class="label">开发者模式</div>
            <div class="hint" style="font-size: 12px">
              打开后会显示导航里的「Mod」——该页面尚未完成，仅作预览
            </div>
          </div>
          <div class="ctrl">
            <label class="switch">
              <input v-model="settings.devMode" type="checkbox" />
              <span class="track" />
              <span class="thumb" />
            </label>
          </div>
        </div>
        <div class="set-row">
          <div>
            <div class="label">未完成功能</div>
            <div class="hint" style="font-size: 12px">
              Mod 页：{{ settings.devMode ? '已在导航中显示' : '已隐藏' }}
            </div>
          </div>
        </div>
      </div>

      <!-- 关于 -->
      <div v-show="settingsTab === 'about'" class="set-group">
        <h3>关于</h3>
        <div class="about-hero">
          <img class="logo" src="../assets/logo.png" alt="YueYue Launcher logo" />
          <div>
            <div class="about-name">YueYue Launcher</div>
            <div class="about-sub">YYL · Mindustry 实例启动器</div>
            <div class="about-maker">月月岛科技</div>
          </div>
        </div>

        <div class="set-row">
          <div class="label">版本</div>
          <div class="ctrl"><span class="val">{{ appVersion || '读取中…' }}</span></div>
        </div>

        <div class="set-row">
          <div class="hint" style="font-size: 12px">
            © 2026 月月岛科技 · YueYue Launcher (YYL)
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
