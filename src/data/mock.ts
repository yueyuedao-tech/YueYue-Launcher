export interface NewsItem {
  date: string
  tag: string
  title: string
}

export const news: NewsItem[] = [
  { date: '2026-10-03', tag: '公告', title: 'YueYue Launcher (YYL) V1 发布：全新二次元界面，支持 Windows 与 Linux' },
  { date: '2026-09-28', tag: '更新', title: '模拟启动流程上线，日志面板实时显示各阶段状态' },
  { date: '2026-09-20', tag: '社区', title: 'Mindustry 游戏管理上线：每个客户端独立存档与配置' },
]
