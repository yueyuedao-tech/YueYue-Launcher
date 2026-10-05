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

/** 中心化服务器索引的单条内容（标签由服务端打好，客户端不参与打标） */
export interface CentralItem {
  id: string
  name: string
  kind: SourceKind
  url: string
  repo: string
  asset: string
  note: string
  group: string
  tags: string[]
  size: number
}

export interface CentralIndex {
  /** remote = 中心化服务器；builtin = 内置兜底 */
  source: string
  /** 非空表示远程失败原因 */
  note: string
  items: CentralItem[]
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
