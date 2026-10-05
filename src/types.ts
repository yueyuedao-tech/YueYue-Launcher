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
export type CentralKind = 'github-repo' | 'direct-url' | 'file-list'

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

/** 中心化服务器索引的单条内容（标签/logo 由服务端下发，客户端不参与打标） */
export interface CentralItem {
  id: string
  name: string
  kind: CentralKind
  url: string
  repo: string
  asset: string
  note: string
  group: string
  tags: string[]
  size: number
  /** 由中心列表服务器下发，用于识别游戏；为空时前端用首字母兜底 */
  logo: string
  /** client = 仅客户端；server 待后续接入 */
  scope: string
}

export interface CentralAsset {
  name: string
  url: string
  size: number
}

export interface CentralVersion {
  tag: string
  title: string
  date: string
  pageUrl: string
  /** 已滤除服务端产物 */
  assets: CentralAsset[]
  /** 被滤掉的服务端资源数 */
  dropped: number
  /** 文件站条目是目录 */
  folder: boolean
}

/** 版本索引：本地缓存 + 同步状态，开机即读、后台再刷 */
export interface VersionIndex {
  syncedAt: number
  ok: number
  total: number
  syncMs: number
  sources: Record<string, CentralVersion[]>
  hashes: Record<string, string>
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
