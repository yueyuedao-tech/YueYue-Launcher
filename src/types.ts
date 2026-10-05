export interface InstanceInfo {
  id: string
  name: string
  createdAt: string
  jarPath: string
  javaPath: string
  dataDir: string
  isolate: boolean
  memoryMb: number
  jvmArgs: string[]
  gameArgs: string[]
  running: boolean
}

export interface InstanceList {
  items: InstanceInfo[]
  skipped: number
}

export interface LaunchLogPayload {
  id: string
  line: string
  stream: string
}

export interface LaunchExitPayload {
  id: string
  code: number
}

export type SourceKind = 'github-repo' | 'direct-url'

export interface SourceItem {
  id: string
  name: string
  kind: SourceKind
  url: string
  repo: string
  asset: string
  note: string
  group: string
  collapsed: boolean
  latestEnabled: boolean
  openInNewPage: boolean
}

export interface GithubVersion {
  tag: string
  title: string
  updated: string
  pageUrl: string
  jarUrl: string
}

export interface DownloadProgressPayload {
  fileName: string
  received: number
  total: number
  percent: number
}

export interface DownloadDonePayload {
  fileName: string
  path: string
}

export interface DownloadErrorPayload {
  fileName: string
  code: number
}

export interface WorkshopItem {
  id: string
  title: string
  url: string
}

export interface ModFile {
  name: string
  size: number
  mtime: string
}

export interface ModLogPayload {
  instanceId: string
  line: string
  stream: string
}

export interface ModDonePayload {
  instanceId: string
  fileId: string
  ok: boolean
  files: string[]
}
