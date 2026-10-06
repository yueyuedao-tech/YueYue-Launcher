<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  autoCollapseLaunch,
  ensureScanned,
  hiddenInstances,
  instances,
  jarScan,
  javaScan,
  loadHidden,
  loadInstances,
  markLaunchFailed,
  markLaunchStopped,
  openLaunchLog,
  resetLaunch,
  selectedInstanceId,
  settings,
  skippedCount,
  store,
} from '../store'
import type { InstanceInfo } from '../types'

const scannedJars = computed(() => jarScan.value)
const JAVA_OPTIONS = computed(() => javaScan.value)

const showForm = ref(false)
const editingId = ref<string | null>(null)
const busy = ref(false)
const startingId = ref<string | null>(null)
const rescanning = ref(false)
const formError = ref('')

const form = reactive({
  name: '',
  jarPath: '',
  javaPath: 'java',
  jvmArgs: '',
  memoryMb: 4096,
  isolate: true,
})

const jarOptions = computed(() => {
  const set = new Set(scannedJars.value)
  if (form.jarPath) set.add(form.jarPath)
  return [...set]
})

function resetForm(info?: InstanceInfo) {
  formError.value = ''
  if (info) {
    editingId.value = info.id
    form.name = info.name
    form.jarPath = info.jarPath
    form.javaPath = info.javaPath
    form.jvmArgs = info.jvmArgs.join(' ')
    form.memoryMb = info.memoryMb
    form.isolate = info.isolate
  } else {
    editingId.value = null
    form.name = ''
    form.jarPath = scannedJars.value[0] ?? ''
    form.javaPath = 'java'
    // 新建游戏带入「个性化 → 新建游戏默认值」里的 JVM 参数
    form.jvmArgs = settings.defaultJvmArgs
    form.memoryMb = settings.memory
    // 新建游戏的隔离默认值跟随「设置 → 启动」的开关
    form.isolate = settings.saveIsolation
  }
  showForm.value = true
}

async function rescan() {
  rescanning.value = true
  try {
    await ensureScanned(true)
  } finally {
    rescanning.value = false
  }
}

/** 游戏名不能重复：重名红字提示并禁止提交（编辑自己时排除自己） */
const nameTaken = computed(() => {
  const n = form.name.trim()
  if (!n) return false
  return instances.value.some((i) => i.name === n && i.id !== editingId.value)
})

async function submit() {
  if (nameTaken.value) {
    formError.value = `已存在同名游戏「${form.name.trim()}」，请换一个名字`
    return
  }
  busy.value = true
  formError.value = ''
  const args = {
    jarPath: form.jarPath.trim(),
    javaPath: form.javaPath.trim() || 'java',
    jvmArgs: form.jvmArgs.split(/\s+/).filter(Boolean),
  }
  try {
    if (editingId.value) {
      const inst = instances.value.find((i) => i.id === editingId.value)
      await invoke('update_instance', {
        id: editingId.value,
        ...args,
        gameArgs: inst?.gameArgs ?? [],
        isolate: form.isolate,
        memoryMb: form.memoryMb,
      })
    } else {
      await invoke('create_instance', {
        name: form.name,
        ...args,
        memoryMb: form.memoryMb,
        isolate: form.isolate,
      })
    }
    showForm.value = false
    await loadInstances()
  } catch (e) {
    formError.value = String(e)
  } finally {
    busy.value = false
  }
}

const pendingDelete = ref<InstanceInfo | null>(null)
/** 与本实例共用同一个 jar 的其它游戏（删除确认里提示） */
const sharedWith = ref<string[]>([])

/** 待删除的游戏：弹出两种删除方式，默认高亮取自设置（避免误删） */
async function askDelete(info: InstanceInfo) {
  pendingDelete.value = info
  sharedWith.value = []
  try {
    // 多个实例共用一个本体时提示一下：本实现不删 jar，但用户需要知道自己删的是哪一个
    sharedWith.value = (await invoke('sharing_jar', { id: info.id })) as string[]
  } catch {
    sharedWith.value = []
  }
}

async function doDelete(mode: 'keep' | 'purge') {
  const info = pendingDelete.value
  if (!info) return
  busy.value = true
  try {
    await invoke('delete_instance', { id: info.id, mode })
    pendingDelete.value = null
    await Promise.all([loadInstances(), loadHidden()])
  } catch (e) {
    window.alert(String(e))
  } finally {
    busy.value = false
  }
}

async function restore(info: InstanceInfo) {
  busy.value = true
  try {
    await invoke('restore_instance', { id: info.id })
    await Promise.all([loadInstances(), loadHidden()])
  } catch (e) {
    window.alert(String(e))
  } finally {
    busy.value = false
  }
}

/** 彻底删除已隐藏的游戏（此时才连存档一起清掉） */
async function purgeHidden(info: InstanceInfo) {
  busy.value = true
  try {
    await invoke('delete_instance', { id: info.id, mode: 'purge' })
    await loadHidden()
  } catch (e) {
    window.alert(String(e))
  } finally {
    busy.value = false
  }
}

async function start(info: InstanceInfo) {
  if (startingId.value) return // 防重入
  if (info.running) {
    // 已在运行：打开已有日志归档，不清空
    openLaunchLog(info.id)
    return
  }
  startingId.value = info.id
  selectedInstanceId.value = info.id
  resetLaunch(info.id)
  try {
    await invoke('launch_instance', { id: info.id })
    autoCollapseLaunch(info.id)
  } catch (e) {
    markLaunchFailed(info.id, String(e))
  } finally {
    startingId.value = null
  }
  await loadInstances()
}

async function stop(info: InstanceInfo) {
  try {
    await invoke('stop_instance', { id: info.id })
    // 后端对被 stop 的实例不发 launch-exit，归档状态要在这里落定
    markLaunchStopped(info.id)
  } catch (e) {
    console.error(e)
  }
  await loadInstances()
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms))
}

function baseName(p: string) {
  return p.split(/[\\/]/).pop() ?? p
}

onMounted(async () => {
  // scan 有结果缓存：仅首次进入游戏页执行，后续切换页面不再重扫
  await Promise.all([loadInstances(), loadHidden(), ensureScanned()])
  if (!editingId.value && !form.jarPath) form.jarPath = jarScan.value[0] ?? ''
})
</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">游戏</h1>
        <p class="page-sub" style="margin-bottom: 0">
          每个游戏独立文件夹（instance.json + launch.config.json + data/），存档互相隔离
        </p>
      </div>
      <div style="display: flex; gap: 10px; align-items: center">
        <button class="btn-ghost" :disabled="rescanning" @click="rescan">
          {{ rescanning ? '扫描中…' : '重新扫描 jar' }}
        </button>
        <button class="btn-grad" style="font-size: 14px; padding: 10px 26px" @click="resetForm()">
          + 新建游戏
        </button>
      </div>
    </div>

    <p v-if="skippedCount > 0" class="page-sub">⚠ 已跳过 {{ skippedCount }} 个损坏的游戏条目</p>

    <!-- 新建/编辑：独立面板（相当于新开一个窗口） -->
    <div v-if="showForm" class="edit-overlay" @click.self="showForm = false">
    <div class="set-group" style="margin-bottom: 18px">
      <h3>{{ editingId ? `编辑游戏：${editingId}` : '新建游戏' }}</h3>
      <div class="set-row">
        <div class="label">游戏名称</div>
        <div class="ctrl">
          <input
            v-model="form.name"
            class="field"
            style="width: 220px"
            :disabled="!!editingId"
            :style="nameTaken ? { borderColor: '#ff8f8f', color: '#ff8f8f' } : undefined"
            placeholder="如：主线服-测试"
          />
          <span v-if="nameTaken" class="meta" style="color: #ff8f8f; font-size: 12px">
            已存在同名游戏
          </span>
        </div>
      </div>
      <div class="set-row">
        <div class="label">游戏 jar<div class="hint">下拉为自动扫描结果，也可手动粘贴路径</div></div>
        <div class="ctrl" style="flex: 1; max-width: 560px">
          <input v-model="form.jarPath" class="field" style="width: 100%" list="jar-options" placeholder="Mindustry.jar 路径" />
          <datalist id="jar-options">
            <option v-for="j in jarOptions" :key="j" :value="j" />
          </datalist>
        </div>
      </div>
      <div class="set-row">
        <div class="label">Java 运行时<div class="hint">每游戏可绑定不同 JRE</div></div>
        <div class="ctrl" style="flex: 1; max-width: 560px">
          <input v-model="form.javaPath" class="field" style="width: 100%" list="java-options" />
          <datalist id="java-options">
            <option v-for="j in JAVA_OPTIONS" :key="j" :value="j" />
          </datalist>
        </div>
      </div>
      <div class="set-row">
        <div class="label">内存 (MB)</div>
        <div class="ctrl">
          <input v-model.number="form.memoryMb" type="range" min="1024" max="16384" step="512" />
          <span class="val">{{ form.memoryMb }} MB</span>
        </div>
      </div>
      <div class="set-row">
        <div class="label">JVM 参数<div class="hint">空格分隔，如 -Dfile.encoding=UTF-8</div></div>
        <div class="ctrl" style="flex: 1; max-width: 460px">
          <input v-model="form.jvmArgs" class="field" style="width: 100%" placeholder="-Xms1G" />
        </div>
      </div>
      <div class="set-row">
        <div class="label">数据隔离<div class="hint">注入 AppData/XDG_DATA_HOME 指向游戏 data/</div></div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="form.isolate" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <p v-if="formError" style="color: #ff7db0; font-size: 13px; padding: 8px 0">{{ formError }}</p>
      <div class="set-row" style="justify-content: flex-end; gap: 10px">
        <button class="btn-ghost" @click="showForm = false">取消</button>
        <button class="btn-grad" style="font-size: 14px; padding: 9px 26px" :disabled="busy" @click="submit">
          {{ editingId ? '保存修改' : '创建' }}
        </button>
      </div>
    </div>
    </div>

    <!-- 删除方式选择 -->
    <div v-if="pendingDelete" class="edit-overlay" @click.self="pendingDelete = null">
      <div class="set-group" style="max-width: 620px; margin: 40px auto">
        <h3>删除「{{ pendingDelete.name }}」</h3>
        <!-- 共用本体提示：多个实例引用同一个 jar 时先说清楚 -->
        <div v-if="sharedWith.length" class="shared-warn">
          <div class="sw-title">⚠ 还有 {{ sharedWith.length }} 个游戏在用同一个本体</div>
          <div class="sw-body">{{ sharedWith.join('、') }}</div>
          <div class="sw-note">
            「删本体留存档」只把这个游戏从列表移到隐藏区，<b>不会删磁盘上的 jar</b>，
            所以其它游戏不受影响；若要彻底删 jar，请先确认没有别的实例还在用它。
          </div>
        </div>
        <div
          class="delete-opt"
          :class="{ sel: settings.defaultDelete === 'keep' }"
          @click="doDelete('keep')"
        >
          <div class="ic">🗂</div>
          <div>
            <div class="ot">删除游戏本体 · 保留存档</div>
            <div class="od">从列表中隐藏（目录移到隐藏区），元数据与存档全部保留，可随时恢复；jar 不动</div>
          </div>
        </div>
        <div
          class="delete-opt"
          :class="{ sel: settings.defaultDelete === 'purge' }"
          @click="doDelete('purge')"
        >
          <div class="ic">🗑</div>
          <div>
            <div class="ot">完全删除 · 含存档</div>
            <div class="od">游戏本体与该目录下所有存档一并清除，不可恢复</div>
          </div>
        </div>
        <div class="set-row">
          <div class="hint">默认方式可在「设置 → 游戏」里更改，避免误删</div>
          <button class="btn-ghost" @click="pendingDelete = null">取消</button>
        </div>
      </div>
    </div>

    <!-- 游戏列表 -->
    <div v-if="instances.length" class="list">
      <div
        v-for="i in instances"
        :key="i.id"
        class="row-card"
        :style="selectedInstanceId === i.id ? { borderColor: 'rgba(255,125,176,.55)' } : undefined"
        @click="selectedInstanceId = i.id"
      >
        <div class="row-icon" :class="{ cy: i.isolate }">{{ i.isolate ? '🗃' : '📂' }}</div>
        <div class="row-main">
          <div class="name">
            {{ i.name }}
            <span v-if="i.running" class="tag" style="margin-left: 8px; background: rgba(255,209,102,.16); color: #ffd166">运行中</span>
          </div>
          <div class="meta">
            {{ baseName(i.jarPath) }} · {{ baseName(i.javaPath) }} · {{ i.memoryMb }}MB · 创建 {{ i.createdAt }}
          </div>
        </div>
        <div class="row-side">
          <button v-if="!i.running" class="btn-grad" style="font-size: 13px; padding: 8px 20px" :disabled="startingId !== null" @click.stop="start(i)">启动</button>
          <button v-else class="btn-ghost" style="color: #ff7db0" @click.stop="stop(i)">停止</button>
          <button class="btn-ghost" @click.stop="resetForm(i)">编辑</button>
          <button class="btn-ghost" style="color: #ff8f8f" @click.stop="askDelete(i)">删除</button>
        </div>
      </div>
    </div>
    <div v-else-if="!showForm" class="empty">
      还没有游戏——点击右上角「新建游戏」，或去「下载」拿一个客户端
    </div>

    <!-- 已隐藏（只删了本体、存档还在） -->
    <template v-if="hiddenInstances.length">
      <div class="section-h" style="margin-top: 22px">已隐藏（保留元数据与存档）</div>
      <div class="list">
        <div v-for="h in hiddenInstances" :key="h.id" class="row-card" style="opacity: .72">
          <div class="row-icon">🗃</div>
          <div class="row-main">
            <div class="name">{{ h.name }} <span class="tag" style="margin-left: 6px">已隐藏</span></div>
            <div class="meta">本体目录已隐藏 · 元数据保留 · {{ baseName(h.jarPath) }}</div>
          </div>
          <div class="row-side">
            <button class="btn-ghost" :disabled="busy" @click="restore(h)">恢复显示</button>
            <button class="btn-ghost" style="color: #ff8f8f" :disabled="busy" @click="purgeHidden(h)">彻底删除</button>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>
