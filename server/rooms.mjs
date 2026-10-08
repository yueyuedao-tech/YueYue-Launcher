import { randomBytes, scryptSync, timingSafeEqual, verify } from 'node:crypto'
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const issuer = (process.env.YYL_SSO_ISSUER || 'https://mindustry.wiki:1090').replace(/\/+$/, '')
const stateFile = process.env.YYL_ROOM_STATE || join(dirname(fileURLToPath(import.meta.url)), '.cache', 'rooms.json')
const cors = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET, POST, DELETE, OPTIONS',
  'access-control-allow-headers': 'authorization, content-type',
  'cache-control': 'no-store',
}
let document = null
let writeQueue = Promise.resolve()
let jwksCache = { expiresAt: 0, keys: [] }

function loadState() {
  if (document) return document
  try {
    document = JSON.parse(readFileSync(stateFile, 'utf8'))
    if (!Array.isArray(document.rooms)) document.rooms = []
    if (!document.identities || typeof document.identities !== 'object') document.identities = {}
  } catch {
    document = { rooms: [], identities: {} }
  }
  return document
}

function saveState() {
  writeQueue = writeQueue.then(() => {
    mkdirSync(dirname(stateFile), { recursive: true })
    const temp = `${stateFile}.${process.pid}.tmp`
    writeFileSync(temp, JSON.stringify(loadState()), { mode: 0o600 })
    renameSync(temp, stateFile)
  })
  return writeQueue
}

async function json(res, status, body) {
  res.writeHead(status, { ...cors, 'content-type': 'application/json; charset=utf-8' })
  res.end(JSON.stringify(body))
}

async function readBody(req) {
  const chunks = []
  let length = 0
  for await (const chunk of req) {
    length += chunk.length
    if (length > 16 * 1024) throw new Error('request too large')
    chunks.push(chunk)
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')
}

async function signingKeys() {
  if (Date.now() < jwksCache.expiresAt) return jwksCache.keys
  const discovery = await fetch(`${issuer}/.well-known/openid-configuration`).then((r) => {
    if (!r.ok) throw new Error('SSO discovery unavailable')
    return r.json()
  })
  const jwks = await fetch(discovery.jwks_uri).then((r) => {
    if (!r.ok) throw new Error('SSO signing keys unavailable')
    return r.json()
  })
  jwksCache = { keys: jwks.keys || [], expiresAt: Date.now() + 10 * 60 * 1000 }
  return jwksCache.keys
}

async function authenticate(req) {
  const match = /^Bearer (.+)$/i.exec(req.headers.authorization || '')
  if (!match) throw Object.assign(new Error('login required'), { status: 401 })
  const token = match[1]
  const parts = token.split('.')
  if (parts.length !== 3) throw Object.assign(new Error('invalid access token'), { status: 401 })
  let header, claims
  try {
    header = JSON.parse(Buffer.from(parts[0], 'base64url').toString('utf8'))
    claims = JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8'))
  } catch {
    throw Object.assign(new Error('invalid access token'), { status: 401 })
  }
  const key = (await signingKeys()).find((candidate) => candidate.kid === header.kid && candidate.kty === 'RSA')
  const valid = key && header.alg === 'RS256' && verify(
    'RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), { key, format: 'jwk' }, Buffer.from(parts[2], 'base64url'),
  )
  if (!valid || claims.iss !== issuer || !claims.sub || Number(claims.exp) <= Date.now() / 1000) {
    throw Object.assign(new Error('invalid or expired access token'), { status: 401 })
  }
  const response = await fetch(`${issuer}/oauth2/profile`, { headers: { authorization: `Bearer ${token}` } })
  if (!response.ok) throw Object.assign(new Error('SSO account is unavailable'), { status: 401 })
  const profile = await response.json()
  return { sub: String(claims.sub), profile: profile.data || {} }
}

function publicRoom(room, user) {
  return {
    id: room.id,
    name: room.name,
    owner: room.ownerName,
    memberCount: room.members.length,
    passwordRequired: !!room.passwordHash,
    mine: room.ownerSub === user.sub,
    joined: room.members.some((member) => member.sub === user.sub),
    members: room.members.map((member) => ({ upid: member.upid, name: member.name, avatar: member.avatar })),
  }
}

function validUpid(value) {
  return typeof value === 'string' && /^[01]{248,256}$/.test(value)
}

function newUpid() {
  const bytes = randomBytes(32)
  const length = 248 + randomBytes(1)[0] % 9
  return [...bytes].map((byte) => byte.toString(2).padStart(8, '0')).join('').slice(0, length)
}

function launcherUpid(state, sub) {
  const value = state.identities[sub]
  return typeof value === 'string' ? value : value?.['yueyue-launcher']
}

function errorStatus(error) {
  return error.status || (error.message === 'request too large' ? 413 : 400)
}

export async function handleRoomRequest(req, res, url) {
  if (!url.pathname.startsWith('/rooms')) return false
  if (req.method === 'OPTIONS') {
    res.writeHead(204, cors).end()
    return true
  }
  if (!url.pathname.startsWith('/rooms')) return false
  try {
    const user = await authenticate(req)
    const state = loadState()
    const segments = url.pathname.split('/').filter(Boolean)
    if (req.method === 'GET' && url.pathname === '/rooms/identity') {
      if (!launcherUpid(state, user.sub)) {
        state.identities[user.sub] = { ...state.identities[user.sub], 'yueyue-launcher': newUpid() }
        await saveState()
      }
      await json(res, 200, { upid: launcherUpid(state, user.sub) })
      return true
    }
    if (req.method === 'GET' && segments.length === 1) {
      await json(res, 200, { rooms: state.rooms.map((room) => publicRoom(room, user)) })
      return true
    }
    if (req.method === 'POST' && segments.length === 1) {
      const body = await readBody(req)
      const name = String(body.name || '').trim().slice(0, 48)
      const password = String(body.password || '')
      const upid = body.upid
      if (!name || !validUpid(upid) || upid !== launcherUpid(state, user.sub) || password.length > 128) throw new Error('invalid room name, password, or UPID')
      if (state.rooms.filter((room) => room.ownerSub === user.sub).length >= 5) {
        throw Object.assign(new Error('each account can create at most five rooms'), { status: 409 })
      }
      const salt = randomBytes(16).toString('hex')
      const room = {
        id: randomBytes(6).toString('base64url'),
        name,
        ownerSub: user.sub,
        ownerUpid: upid,
        ownerName: String(user.profile.display_name || user.profile.username || 'Mindustry 玩家').slice(0, 80),
        passwordSalt: salt,
        passwordHash: password ? scryptSync(password, salt, 32).toString('hex') : '',
        createdAt: Date.now(),
        networkName: `yyl-${randomBytes(12).toString('hex')}`,
        networkSecret: randomBytes(32).toString('base64url'),
        members: [{ sub: user.sub, upid, name: String(user.profile.display_name || user.profile.username || 'Mindustry 玩家').slice(0, 80), avatar: String(user.profile.avatar_url || '').slice(0, 500) }],
      }
      state.rooms.unshift(room)
      await saveState()
      await json(res, 201, { room: publicRoom(room, user), network: { name: room.networkName, secret: room.networkSecret } })
      return true
    }
    const roomId = segments[1]
    const room = state.rooms.find((item) => item.id === roomId)
    if (!room) throw Object.assign(new Error('room not found'), { status: 404 })
    if (req.method === 'GET' && segments.length === 3 && segments[2] === 'network') {
      if (!room.members.some((member) => member.sub === user.sub)) {
        throw Object.assign(new Error('not a room member'), { status: 403 })
      }
      await json(res, 200, { network: { name: room.networkName, secret: room.networkSecret } })
      return true
    }
    if (req.method === 'DELETE' && segments.length === 2) {
      if (room.ownerSub !== user.sub) throw Object.assign(new Error('only the room owner can delete it'), { status: 403 })
      state.rooms = state.rooms.filter((item) => item !== room)
      await saveState()
      await json(res, 200, { ok: true })
      return true
    }
    if (req.method === 'POST' && segments.length === 3 && segments[2] === 'join') {
      const body = await readBody(req)
      if (!validUpid(body.upid) || body.upid !== launcherUpid(state, user.sub)) throw new Error('invalid UPID')
      if (room.passwordHash) {
        const candidate = scryptSync(String(body.password || ''), room.passwordSalt, 32)
        const expected = Buffer.from(room.passwordHash, 'hex')
        if (candidate.length !== expected.length || !timingSafeEqual(candidate, expected)) {
          throw Object.assign(new Error('incorrect room password'), { status: 403 })
        }
      }
      const existing = room.members.find((member) => member.sub === user.sub)
      if (!existing) room.members.push({
        sub: user.sub,
        upid: body.upid,
        name: String(user.profile.display_name || user.profile.username || 'Mindustry 玩家').slice(0, 80),
        avatar: String(user.profile.avatar_url || '').slice(0, 500),
      })
      await saveState()
      await json(res, 200, { room: publicRoom(room, user), network: { name: room.networkName, secret: room.networkSecret } })
      return true
    }
    if (req.method === 'DELETE' && segments.length === 3 && segments[2] === 'membership') {
      if (room.ownerSub === user.sub) throw Object.assign(new Error('room owner cannot leave; delete the room instead'), { status: 409 })
      room.members = room.members.filter((member) => member.sub !== user.sub)
      await saveState()
      await json(res, 200, { ok: true })
      return true
    }
    if (req.method === 'DELETE' && segments.length === 4 && segments[2] === 'members') {
      if (room.ownerSub !== user.sub) throw Object.assign(new Error('only the room owner can remove members'), { status: 403 })
      const target = room.members.find((member) => member.upid === segments[3])
      if (!target) throw Object.assign(new Error('member not found'), { status: 404 })
      if (target.sub === room.ownerSub) throw new Error('room owner cannot be removed')
      room.members = room.members.filter((member) => member !== target)
      room.networkSecret = randomBytes(32).toString('base64url')
      await saveState()
      await json(res, 200, { ok: true })
      return true
    }
    throw Object.assign(new Error('not found'), { status: 404 })
  } catch (error) {
    await json(res, errorStatus(error), { error: error.message || 'room request failed' })
    return true
  }
}
