<script setup lang="ts">
import { computed } from 'vue'
import { ACCENT_PAIRS, ensureNotifyPermission, settings, settingsNavPos } from '../store'

/** 打开提醒开关时顺便申请通知权限（拒绝了也不影响任务栏闪烁） */
function onNotifyToggle(e: Event) {
  if ((e.target as HTMLInputElement).checked) void ensureNotifyPermission()
}

/** 设置页分类导航位置：读取时给默认值（跟随主导航），写入时才落盘为独立设置 */
const settingsNav = computed<'left' | 'top'>({
  get: () => settingsNavPos.value,
  set: (v) => {
    settings.settingsNavPosition = v
  },
})

/** 自定义背景图：先压到 1600 宽的 JPEG 再存，避免撑爆 localStorage 配额 */
function onBgFile(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  const url = URL.createObjectURL(file)
  const img = new Image()
  img.onload = () => {
    const scale = Math.min(1, 1600 / Math.max(1, img.width))
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(img.width * scale))
    canvas.height = Math.max(1, Math.round(img.height * scale))
    const ctx = canvas.getContext('2d')
    if (ctx) {
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
      settings.bgImage = canvas.toDataURL('image/jpeg', 0.78)
    }
    URL.revokeObjectURL(url)
  }
  img.onerror = () => URL.revokeObjectURL(url)
  img.src = url
  input.value = ''
}

function clearBg() {
  settings.bgImage = ''
}
</script>

<template>
  <div class="settings-grid">
    <div class="set-group">
      <h3>导航位置</h3>
      <div class="set-row">
        <div>
          <div class="label">主导航栏位置</div>
        </div>
        <div class="ctrl" style="gap: 8px">
          <label class="btn-ghost" :style="settings.navPosition === 'left' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settings.navPosition" type="radio" name="main-nav-pos" value="left" style="margin-right: 6px" />左侧
          </label>
          <label class="btn-ghost" :style="settings.navPosition === 'top' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settings.navPosition" type="radio" name="main-nav-pos" value="top" style="margin-right: 6px" />顶部
          </label>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">设置页导航位置</div>
        </div>
        <div class="ctrl" style="gap: 8px">
          <label class="btn-ghost" :style="settingsNav === 'left' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settingsNav" type="radio" name="settings-nav-pos" value="left" style="margin-right: 6px" />左侧
          </label>
          <label class="btn-ghost" :style="settingsNav === 'top' ? { borderColor: 'var(--pink)', color: 'var(--pink)' } : undefined">
            <input v-model="settingsNav" type="radio" name="settings-nav-pos" value="top" style="margin-right: 6px" />顶部
          </label>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">启动页</div>
          <div class="hint" style="font-size: 12px">启动器打开时停在哪个页面</div>
        </div>
        <div class="ctrl">
          <select v-model="settings.startView" class="field">
            <option value="home">首页</option>
            <option value="instances">游戏</option>
            <option value="downloads">下载</option>
            <option value="settings">设置</option>
          </select>
        </div>
      </div>
    </div>

    <div class="set-group">
      <h3>外观</h3>
      <div class="set-row">
        <div>
          <div class="label">自定义背景图</div>
        </div>
        <div class="ctrl" style="gap: 8px">
          <label class="btn-ghost" style="position: relative; overflow: hidden">
            选择图片
            <input type="file" accept="image/*" style="position: absolute; inset: 0; opacity: 0; cursor: pointer" @change="onBgFile" />
          </label>
          <button class="btn-ghost" @click="clearBg">恢复默认</button>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">主题</div>
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
          <div class="hint" style="font-size: 12px">选「最小化到托盘」后可从托盘图标恢复</div>
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
      <h3>界面</h3>
      <div class="set-row">
        <div>
          <div class="label">界面缩放</div>
        </div>
        <div class="ctrl">
          <select v-model.number="settings.uiScale" class="field">
            <option :value="90">90%</option>
            <option :value="100">100%</option>
            <option :value="110">110%</option>
            <option :value="125">125%</option>
            <option :value="150">150%</option>
          </select>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">背景模糊</div>
        </div>
        <div class="ctrl">
          <input v-model.number="settings.bgBlur" type="range" min="0" max="30" step="1" />
          <span class="val">{{ settings.bgBlur }} px</span>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">界面动画</div>
          <div class="hint" style="font-size: 12px">关掉后过渡与呼吸圈等动效全部停用</div>
        </div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="settings.animations" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
    </div>

    <div class="set-group">
      <h3>提醒与日志</h3>
      <div class="set-row">
        <div>
          <div class="label">下载完成提醒</div>
          <div class="hint" style="font-size: 12px">任务栏闪烁 +（允许时）系统通知</div>
        </div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="settings.downloadNotify" type="checkbox" @change="onNotifyToggle" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
      <div class="set-row">
        <div>
          <div class="label">游戏退出时自动弹日志</div>
        </div>
        <div class="ctrl">
          <label class="switch">
            <input v-model="settings.autoOpenLogOnExit" type="checkbox" />
            <span class="track" />
            <span class="thumb" />
          </label>
        </div>
      </div>
    </div>
  </div>
</template>
