import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { account, bearerToken } from './account'

const base = 'https://mindustry.wiki:1200'
const connected = new Map<string, string>()
let timer: number | undefined

export interface RoomMember { upid: string; name: string; avatar: string }
export interface Room {
  id: string
  name: string
  owner: string
  memberCount: number
  passwordRequired: boolean
  mine: boolean
  joined: boolean
  members: RoomMember[]
}

export const roomState = reactive({ rooms: [] as Room[], error: '', busy: false })

export async function roomRequest(path: string, init: RequestInit = {}) {
  if (!bearerToken()) throw new Error('请先登录 SSO')
  const response = await fetch(`${base}${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${bearerToken()}`, ...(init.headers || {}) },
  })
  const body = await response.json()
  if (!response.ok) throw new Error(body.error || '房间服务请求失败')
  return body
}

export async function refreshRooms() {
  if (!account.profile) return
  try {
    roomState.rooms = (await roomRequest('/rooms')).rooms
    const joined = new Set(roomState.rooms.filter((room) => room.joined).map((room) => room.id))
    for (const id of connected.keys()) {
      if (!joined.has(id)) {
        await invoke('disconnect_room', { roomId: id })
        connected.delete(id)
      }
    }
    for (const id of joined) {
      const body = await roomRequest(`/rooms/${encodeURIComponent(id)}/network`)
      const secret = body.network.secret as string
      if (connected.get(id) !== secret) {
        await invoke('connect_room', { roomId: id, networkName: body.network.name, networkSecret: secret, upid: account.upid })
        connected.set(id, secret)
      }
    }
    roomState.error = ''
  } catch (error) {
    roomState.error = String(error)
  }
}

export function startRoomSync() {
  if (timer) return
  void refreshRooms()
  timer = window.setInterval(() => { void refreshRooms() }, 10000)
}

export function stopRoomSync() {
  if (timer) clearInterval(timer)
  timer = undefined
  connected.clear()
  roomState.rooms = []
  void invoke('disconnect_all_rooms')
}
