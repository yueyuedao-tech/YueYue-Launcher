import { reactive } from 'vue'
import { open } from '@tauri-apps/plugin-shell'

const issuer = 'https://mindustry.wiki:1090'
const clientId = 'yueyue-launcher'
const redirectUri = 'com.starlight.launcher:/oauth2redirect'
const tokenKey = 'yyl-sso-refresh-token'

export interface AccountProfile {
  username: string
  display_name: string
  avatar_url: string
  coins: number
  experience: number
  level: { level: number; next_level: number; progress_pct: number; next_exp: number }
}

export const account = reactive({
  profile: null as AccountProfile | null,
  upid: '',
  busy: false,
  error: '',
})

let accessToken = ''
let callbackPending: ((url: URL) => void) | null = null

function randomString(bytes = 32) {
  const data = crypto.getRandomValues(new Uint8Array(bytes))
  return btoa(String.fromCharCode(...data)).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

export function initializeAccount() {
  account.upid = ''
}

export function handleLoginUrl(raw: string) {
  let url: URL
  try {
    url = new URL(raw)
  } catch {
    return
  }
  if (url.protocol !== 'com.starlight.launcher:' || url.pathname !== '/oauth2redirect') return
  callbackPending?.(url)
}

async function fetchProfile() {
  if (!accessToken) return
  const response = await fetch(`${issuer}/oauth2/profile`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  })
  if (!response.ok) throw new Error(`SSO 资料读取失败 (${response.status})`)
  const body = await response.json()
  const identity = await fetch('https://mindustry.wiki:1200/rooms/identity', {
    headers: { Authorization: `Bearer ${accessToken}` },
  })
  if (!identity.ok) throw new Error(`UPID 读取失败 (${identity.status})`)
  account.upid = (await identity.json()).upid
  account.profile = body.data as AccountProfile
}

async function exchange(params: URLSearchParams) {
  const response = await fetch(`${issuer}/oauth2/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: params,
  })
  const body = await response.json()
  if (!response.ok) throw new Error(body.error_description || body.error || 'SSO 授权失败')
  accessToken = body.access_token
  if (body.refresh_token) sessionStorage.setItem(tokenKey, body.refresh_token)
  await fetchProfile()
}

export async function login() {
  account.busy = true
  account.error = ''
  try {
    const state = randomString(24)
    const verifier = randomString(48)
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier))
    const challenge = btoa(String.fromCharCode(...new Uint8Array(digest)))
      .replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
    const auth = new URL(`${issuer}/oauth2/authorize`)
    auth.search = new URLSearchParams({
      response_type: 'code',
      client_id: clientId,
      redirect_uri: redirectUri,
      scope: 'openid profile email offline_access',
      state,
      code_challenge: challenge,
      code_challenge_method: 'S256',
    }).toString()
    const callback = new Promise<URL>((resolve, reject) => {
      const timeout = window.setTimeout(() => {
        callbackPending = null
        reject(new Error('等待 SSO 登录回调超时'))
      }, 5 * 60 * 1000)
      callbackPending = (url) => {
        clearTimeout(timeout)
        callbackPending = null
        resolve(url)
      }
    })
    await open(auth.toString())
    const url = await callback
    if (url.searchParams.get('state') !== state) throw new Error('SSO state 校验失败')
    const code = url.searchParams.get('code')
    if (!code) throw new Error(url.searchParams.get('error_description') || 'SSO 未返回授权码')
    await exchange(new URLSearchParams({
      grant_type: 'authorization_code',
      client_id: clientId,
      code,
      redirect_uri: redirectUri,
      code_verifier: verifier,
    }))
  } catch (error) {
    account.error = String(error)
    throw error
  } finally {
    account.busy = false
  }
}

export async function restoreAccount() {
  const refreshToken = sessionStorage.getItem(tokenKey)
  if (!refreshToken) return
  try {
    await exchange(new URLSearchParams({
      grant_type: 'refresh_token',
      client_id: clientId,
      refresh_token: refreshToken,
    }))
  } catch {
    sessionStorage.removeItem(tokenKey)
    accessToken = ''
    account.profile = null
  }
}

export function logout() {
  accessToken = ''
  account.profile = null
  account.upid = ''
  sessionStorage.removeItem(tokenKey)
}

export function bearerToken() {
  return accessToken
}
