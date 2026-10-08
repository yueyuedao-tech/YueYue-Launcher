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

/** 中心化服务器索引的单条内容（标签/logo/标语/版本均由服务端下发，客户端只读） */
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
  /** 标语：一句话说明这个游戏端是什么；服务端优先，客户端有兜底文案 */
  slogan: string
  /** client = 仅客户端；server 待后续接入 */
  scope: string
  /** 服务端标注好的版本快照；非空时客户端不再直连 GitHub（403 消失的落点） */
  versions?: CentralVersion[]
  /** 服务端标注版本失败的原因 */
  versionsError?: string
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
  /** 源 id → 本次拉取失败原因（成功时清除） */
  errors: Record<string, string>
  /** 中心下发的镜像清单（跟着缓存一起留着） */
  mirrors?: CentralMirrors
  /** 中心下发的每日信息 Markdown（跟着缓存一起留着） */
  infoBar?: string
}

/** 中心下发的镜像条目（工坊镜像等） */
export interface MirrorItem {
  name: string
  url: string
}

export interface CentralMirrors {
  workshop: MirrorItem[]
}

export interface CentralIndex {
  /** remote = 中心化服务器；builtin = 内置兜底 */
  source: string
  /** 非空表示远程失败原因 */
  note: string
  items: CentralItem[]
  /** 中心下发的镜像清单 */
  mirrors: CentralMirrors
  /** 每日信息：Markdown 原文（首页底部公告） */
  infoBar?: string
  /** 启动器自身最新版本（关于页「检查更新」用，由中心化服务器下发） */
  launcher?: LauncherInfo
}

/** 启动器版本信息 */
export interface LauncherInfo {
  version: string
  url: string
  note: string
}

export interface DownloadProgressPayload {
  fileName: string
  received: number
  total: number
  percent: number
  /** 字节/秒（指数平滑） */
  speed: number
  /** 实际使用的连接数（服务器不支持分段时为 1） */
  threads: number
}

/** 下载任务快照（来自后端 list_downloads，抽屉面板的唯一真相） */
export interface DownloadTask {
  fileName: string
  name: string
  url: string
  status: 'downloading' | 'done' | 'error'
  received: number
  total: number
  percent: number
  speed: number
  threads: number
  path: string
  code: number
  startedAt: number
  /** 直接复用本地已有客户端，没有真的下载 */
  reused?: boolean
}

export interface DownloadDonePayload {
  fileName: string
  path: string
  /** 发起下载时带的唯一标识，原样回传（同一个文件可能被下载多次） */
  token?: string
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

export interface ModCatalogItem {
  repo: string
  internalName: string
  name: string
  author: string
  lastUpdated: string
  stars: number
  version: string
  minGameVersion: string
  hasJava: boolean
  iconUrl: string
  description: string
}

export interface ModCatalogPage {
  items: ModCatalogItem[]
  total: number
}

export interface ModDownloadInfo {
  fileName: string
  url: string
}

export interface MapItem {
  id: number
  name: string
  desc: string
  preview: string
  tags: string[]
  width: number
  height: number
  mode: string
}

export interface MapPage {
  items: MapItem[]
  hasMore: boolean
}
