<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  ensureScanned,
  instances,
  jarScan,
  javaScan,
  loadInstances,
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
    form.jvmArgs = ''
    form.memoryMb = settings.memory
    form.isolate = true
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

async function submit() {
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

async function remove(info: InstanceInfo) {
  if (!window.confirm(`删除实例「${info.name}」？其数据目录 data/ 将一并删除，不可恢复。`)) return
  busy.value = true
  try {
    await invoke('delete_instance', { id: info.id })
    await loadInstances()
  } catch (e) {
    window.alert(String(e))
  } finally {
    busy.value = false
  }
}

async function start(info: InstanceInfo) {
  if (startingId.value) return // 防重入
  startingId.value = info.id
  selectedInstanceId.value = info.id
  resetLaunch(info.id)
  await sleep(60) // 等浮层挂载并注册日志监听
  try {
    await invoke('launch_instance', { id: info.id })
  } catch (e) {
    store.launch.logs.push({ text: `[错误] ${e}`, cls: 'warn' })
    store.launch.status = 'failed'
  } finally {
    startingId.value = null
  }
  await loadInstances()
}

async function stop(info: InstanceInfo) {
  try {
    await invoke('stop_instance', { id: info.id })
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
  // scan 有结果缓存：仅首次进入实例页执行，后续切换页面不再重扫
  await Promise.all([loadInstances(), ensureScanned()])
  if (!editingId.value && !form.jarPath) form.jarPath = jarScan.value[0] ?? ''
})
</script>

<template>
  <section class="page">
    <div class="toolbar" style="justify-content: space-between">
      <div>
        <h1 class="page-title">实例管理</h1>
        <p class="page-sub" style="margin-bottom: 0">
          每个实例独立文件夹（instance.json + launch.config.json + data/），存档设置互相隔离
        </p>
      </div>
      <div style="display: flex; gap: 10px; align-items: center">
        <button class="btn-ghost" :disabled="rescanning" @click="rescan">
          {{ rescanning ? '扫描中…' : '重新扫描 jar' }}
        </button>
        <button class="btn-grad" style="font-size: 14px; padding: 10px 26px" @click="resetForm()">
          + 新建实例
        </button>
      </div>
    </div>

    <p v-if="skippedCount > 0" class="page-sub">⚠ 已跳过 {{ skippedCount }} 个损坏的实例条目</p>

    <!-- 新建/编辑表单 -->
    <div v-if="showForm" class="set-group" style="margin-bottom: 18px">
      <h3>{{ editingId ? `编辑实例：${editingId}` : '新建实例' }}</h3>
      <div class="set-row">
        <div class="label">实例名称</div>
        <div class="ctrl">
          <input v-model="form.name" class="field" style="width: 220px" :disabled="!!editingId" placeholder="如：主线服-测试" />
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
        <div class="label">Java 运行时<div class="hint">每实例可绑定不同 JRE</div></div>
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
        <div class="label">数据隔离<div class="hint">注入 AppData/XDG_DATA_HOME 指向实例 data/</div></div>
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

    <!-- 实例列表 -->
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
          <button class="btn-ghost" style="color: #ff8f8f" @click.stop="remove(i)">删除</button>
        </div>
      </div>
    </div>
    <div v-else-if="!showForm" class="empty">
      还没有实例——点击右上角「新建实例」创建第一个客户端
    </div>
  </section>
</template>
