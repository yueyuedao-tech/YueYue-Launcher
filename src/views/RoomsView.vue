<script setup lang="ts">
import { ref } from 'vue'
import { account, login } from '../account'
import { roomRequest, roomState, refreshRooms, type Room, type RoomMember } from '../rooms'

const name = ref('')
const password = ref('')
const joinPassword = ref<Record<string, string>>({})

async function create() {
  roomState.busy = true
  try {
    await roomRequest('/rooms', { method: 'POST', body: JSON.stringify({ name: name.value, password: password.value, upid: account.upid }) })
    name.value = ''; password.value = ''; await refreshRooms()
  } catch (error) { roomState.error = String(error) } finally { roomState.busy = false }
}

async function join(room: Room) {
  try {
    await roomRequest(`/rooms/${encodeURIComponent(room.id)}/join`, { method: 'POST', body: JSON.stringify({ password: joinPassword.value[room.id] || '', upid: account.upid }) })
    await refreshRooms()
  } catch (error) { roomState.error = String(error) }
}

async function change(path: string) {
  try {
    await roomRequest(path, { method: 'DELETE' })
    await refreshRooms()
  } catch (error) { roomState.error = String(error) }
}

function kick(room: Room, member: RoomMember) {
  void change(`/rooms/${encodeURIComponent(room.id)}/members/${encodeURIComponent(member.upid)}`)
}
</script>

<template>
  <section class="page">
    <div class="page-head"><div><div class="eyebrow">NETWORK</div><h1>房间</h1></div><button class="btn" @click="refreshRooms">刷新</button></div>
    <div v-if="!account.profile" class="empty-state"><button class="btn primary" @click="login">登录 SSO</button></div>
    <template v-else>
      <div class="set-group room-create">
        <h3>创建房间</h3>
        <div class="toolbar"><input v-model="name" class="field" maxlength="48" placeholder="房间名称" /><input v-model="password" class="field" type="password" maxlength="128" placeholder="密码（可选）" /><button class="btn primary" :disabled="roomState.busy || !name.trim()" @click="create">创建</button></div>
      </div>
      <div class="room-grid">
        <article v-for="room in roomState.rooms" :key="room.id" class="room-card">
          <div class="room-card-head"><strong>{{ room.name }}</strong><span class="tag">{{ room.memberCount }} 人</span></div>
          <div class="meta">房主：{{ room.owner }} · {{ room.passwordRequired ? '需要密码' : '无需密码' }}</div>
          <div class="room-members"><span v-for="member in room.members" :key="member.upid" class="member-chip"><img v-if="member.avatar" :src="member.avatar" alt="" />{{ member.name }}</span></div>
          <div class="toolbar" v-if="!room.joined"><input v-if="room.passwordRequired" v-model="joinPassword[room.id]" class="field" type="password" placeholder="房间密码" /><button class="btn primary" @click="join(room)">加入</button></div>
          <div class="room-actions" v-else><span class="tag" style="color:var(--cyan)">已加入</span><button v-if="room.mine" class="btn danger" @click="change(`/rooms/${encodeURIComponent(room.id)}`)">删除房间</button><button v-else class="btn" @click="change(`/rooms/${encodeURIComponent(room.id)}/membership`)">退出房间</button><button v-if="room.mine" v-for="member in room.members.filter((m) => m.upid !== account.upid)" :key="member.upid" class="btn danger" @click="kick(room, member)">踢出 {{ member.name }}</button></div>
        </article>
      </div>
    </template>
    <p v-if="roomState.error" class="error-text">{{ roomState.error }}</p>
  </section>
</template>
