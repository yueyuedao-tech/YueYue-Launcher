export interface GameVersion {
  id: string
  name: string
  type: '正式版' | '快照版' | '整合包'
  lastPlayed: string
  size: string
}

export interface Downloadable {
  id: string
  name: string
  desc: string
  type: '正式版' | '快照版' | '整合包'
  size: string
}

export interface NewsItem {
  date: string
  tag: string
  title: string
}

export const versions: GameVersion[] = [
  { id: 'v1214', name: '1.21.4 星穹生存', type: '整合包', lastPlayed: '今天 18:42', size: '1.8 GB' },
  { id: 'v1214s', name: '1.21.4', type: '正式版', lastPlayed: '昨天 21:05', size: '486 MB' },
  { id: 'v1206', name: '1.20.6 光影电影', type: '整合包', lastPlayed: '3 天前', size: '2.4 GB' },
  { id: 'v1204', name: '1.20.4', type: '正式版', lastPlayed: '上周', size: '412 MB' },
  { id: 'v25w03a', name: '25w03a', type: '快照版', lastPlayed: '未启动', size: '430 MB' },
  { id: 'v1165', name: '1.16.5 怀旧冒险', type: '整合包', lastPlayed: '上月', size: '1.2 GB' },
]

export const downloadables: Downloadable[] = [
  { id: 'd1', name: 'Minecraft 1.21.4', desc: '正式版 · Tricky Trials 后续修复', type: '正式版', size: '486 MB' },
  { id: 'd2', name: 'Minecraft 25w03a', desc: '快照版 · 下一版本实验性内容', type: '快照版', size: '430 MB' },
  { id: 'd3', name: '星穹生存 2.3', desc: '整合包 · 科技 + 星空冒险', type: '整合包', size: '1.8 GB' },
  { id: 'd4', name: '光影电影 4.0', desc: '整合包 · 极致光影与材质', type: '整合包', size: '2.4 GB' },
  { id: 'd5', name: 'Minecraft 1.20.6', desc: '正式版 · 稳定长期支持', type: '正式版', size: '449 MB' },
  { id: 'd6', name: 'Create: 创造工坊', desc: '整合包 · 机械动力建造', type: '整合包', size: '968 MB' },
]

export const news: NewsItem[] = [
  { date: '2026-10-03', tag: '公告', title: '星启启动器 V1 发布：全新二次元界面，支持 Windows 与 Linux' },
  { date: '2026-09-28', tag: '更新', title: '模拟启动流程上线，日志面板实时显示各阶段状态' },
  { date: '2026-09-20', tag: '社区', title: '星穹生存 2.3 整合包入选本周推荐榜单' },
]
