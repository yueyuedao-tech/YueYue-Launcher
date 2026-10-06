#!/usr/bin/env node
/**
 * YYL 中心化服务器 —— 极简、零依赖
 *
 * 职责：索引源内容 → 打标（类型 / 分组 / 附加标签）→ 下发 logo + 标语 + 版本列表 → 输出 index.json
 *
 * 标注分层（用户决策 2026-10-05）：**服务端与客户端都打标**，且两套规则必须一致，
 * 否则同一个源「服务器在线 / 离线」会看到两套标签。
 *   - 服务端（本文件）：权威来源。index.json 里的 tags / slogan / logo / versions 都在这里生成。
 *   - 客户端（src-tauri/src/central.rs）：服务器不可用退回内置索引时，用同一套规则本地补标 + 标语兜底。
 *
 * 403 的根因与解法（2026-10-05 实测）：
 *   本机直连 api.github.com → 200；经设置里的代理 127.0.0.1:7897 → **403**
 *   （代理出口 IP 被 GitHub 限流/拦截）。客户端一旦自己打 api.github.com 就会踩这个 403。
 *   因此 GitHub releases 索引整体搬到服务端：客户端只读 {base}/index.json 里标注好的 versions，
 *   不再直连 GitHub；服务端侧有 TTL 缓存 + 可选 GITHUB_TOKEN，60 次/小时的上限也就打不满了。
 *   file-list（HTML 目录页）没有 API 限额，仍由客户端解析，服务端不下发：
 *   条目 versions 为空且无 versionsError 时，客户端才走自己的网络逻辑。
 *
 * 用法：
 *   node server/central.mjs build            生成 server/index.json
 *   node server/central.mjs serve [端口]     起服务（默认 8787），每次请求前重新生成（版本走缓存）
 *
 * 环境变量：
 *   GITHUB_TOKEN       提高 GitHub 限额（未带 token 时 60 次/小时，多仓库很容易打满）
 *   YYL_GH_TTL_MS      GitHub 版本缓存有效期，默认 15 分钟（0 = 每次重拉）
 *   NODE_USE_ENV_PROXY=1  让 Node 的 fetch 走 HTTPS_PROXY/HTTP_PROXY（Node 24+）
 *
 * 客户端把「设置 → 下载 → 中心化服务器」填成 http://127.0.0.1:8787 即可。
 * 部署到任意静态托管时，只要能访问到 index.json 就行（build 产物自带版本快照）。
 */
import { createServer } from 'node:http'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const OUT = join(HERE, 'index.json')
const CACHE_DIR = join(HERE, '.cache')
/** 2 = 增加 slogan（标语）；3 仍未启用。客户端按带默认值的宽松解析，旧版读 v2 也不会崩 */
const SCHEMA = 2
const GH_TTL_MS = Number(process.env.YYL_GH_TTL_MS ?? 15 * 60 * 1000)
const GH_PAGES = 5

/** 源内容清单：只写事实，标签由 build 阶段统一派生；logo/标语随源一起下发。
 *  id 显式写死并与客户端内置索引（central.rs builtin_items）一致：
 *  否则「服务器在线 / 离线」两套 id 会让版本缓存对不上，标语兜底也匹配不到。 */
const SOURCES = [
  {
    id: 'src-mindustry-official',
    name: 'Mindustry 官方',
    kind: 'github-repo',
    repo: 'Anuken/Mindustry',
    url: 'https://github.com/Anuken/Mindustry/releases',
    group: '中心',
    note: 'Anuken/Mindustry 官方 releases',
    slogan: 'Anuken 官方原版 · 紧跟上游 stable',
    tags: ['官方'],
    scope: 'client',
    logo: 'https://github.com/Anuken.png?size=96',
  },
  {
    id: 'src-mdtbbs-v8',
    name: 'Mindustry v8',
    kind: 'file-list',
    url: 'https://file.mdtbbs.cn/category/Mindustry/v8',
    group: '中心',
    note: 'mdtbbs 文件站分类目录',
    slogan: 'mdtbbs 文件站 · 国内直连目录',
    tags: ['v8'],
    scope: 'client',
    logo: 'https://file.mdtbbs.cn/favicon.svg',
  },
  {
    id: 'src-minedx',
    name: 'MindustryX',
    kind: 'github-repo',
    repo: 'TinyLake/MindustryX',
    url: 'https://github.com/TinyLake/MindustryX/releases',
    group: '中心',
    note: 'TinyLake/MindustryX releases',
    slogan: 'TinyLake 分支 · 桌面端增强整合',
    tags: ['GitHub'],
    scope: 'client',
    logo: 'https://github.com/TinyLake.png?size=96',
  },
]

const KIND_TAG = { 'direct-url': '直链', 'github-repo': '仓库', 'file-list': '目录' }

/**
 * 中心下发的镜像清单（客户端不再手填地址）。
 * 工坊镜像：填 name + url（url 是拼在 steamcommunity.com 前面的前缀，留空表示直连）。
 * 例如：{ name: '示例工坊镜像', url: 'https://your-steam-mirror/' }
 */
const MIRRORS = {
  workshop: [],
}

/**
 * 每日信息（首页底部那三条公告）的**内置默认值**。
 * 实际下发的内容优先取 `content/info.md`（见 readInfoBar）：那个文件可以在服务器上
 * 随时改，改完下一次请求就生效，不用重建镜像。只有它不存在/为空时才回落到这里。
 *
 * 格式：Markdown，一行一条；行首 `**日期 · 标签**` 之后是标题。
 */
const INFO_BAR = [
  '**2026-10-03 · 公告** YueYue Launcher (YYL) V1 发布：全新二次元界面，支持 Windows 与 Linux',
  '**2026-09-28 · 更新** 模拟启动流程上线，日志面板实时显示各阶段状态',
  '**2026-09-20 · 社区** Mindustry 游戏管理上线：每个客户端独立存档与配置',
].join('\n')

/** 可编辑内容目录：compose 里会把它挂到宿主机，方便直接改公告 */
const CONTENT_DIR = join(HERE, 'content')
const INFO_FILE = join(CONTENT_DIR, 'info.md')

/**
 * 取当前要下发的每日信息：优先 `content/info.md`（可编辑），否则用内置默认值。
 * 每次请求都读盘 —— 改完文件不用重启容器。
 */
function readInfoBar() {
  try {
    if (existsSync(INFO_FILE)) {
      const text = readFileSync(INFO_FILE, 'utf8').replace(/^\uFEFF/, '').trim()
      if (text) return text.slice(0, 8000)
    }
  } catch {
    // 读不到（权限/编码等）就回落到内置内容，保证接口不会因此 500
  }
  return INFO_BAR
}

const clean = (s, n) => String(s ?? '').trim().slice(0, n)
const slug = (s, i) =>
  clean(s, 48).toLowerCase().replace(/[^a-z0-9一-龥]+/g, '-').replace(/^-|-$/g, '') || `item-${i}`

/** 区分打标签：类型标签 + 分组标签 + 条目自带标签，去重后封顶 8 个
 *  客户端 central.rs 的 derive_tags 必须与这里保持一致 */
function tagsOf(src) {
  const list = [KIND_TAG[src.kind], src.group, ...(src.tags ?? [])]
  return [...new Set(list.filter(Boolean).map((t) => clean(t, 16)))].slice(0, 8)
}

/* ================= 服务端版本标注：客户端不再直连 api.github.com ================= */

/** 与服务端一致的「服务端产物」判定：Rust 侧 is_server_asset 必须同规则 */
function isServerAsset(name) {
  const n = String(name ?? '').toLowerCase()
  return n.startsWith('server') || n.startsWith('dedicated') || n.includes('-server')
}

/** 客户端可下载资产：名称**严格以「Mindustry」开头**、排除 apk、排除服务端产物。
 *  与 src-tauri/src/central.rs 的 is_client_asset 同一套规则（两边都打标）。
 *  `dependencies.jar` / `assets.jar` / `desktop-release.jar` / `dexed-*.loader.jar`
 *  这些非客户端本体的包不进下载按钮。 */
function isClientAsset(name) {
  const n = String(name ?? '')
  if (!n.startsWith('Mindustry')) return false
  return !n.toLowerCase().endsWith('.apk') && !isServerAsset(n)
}

function cachePath(repo) {
  return join(CACHE_DIR, `${repo.replace(/[^A-Za-z0-9_.-]/g, '_')}.json`)
}

function readCache(repo) {
  if (GH_TTL_MS <= 0) return null
  try {
    const j = JSON.parse(readFileSync(cachePath(repo), 'utf8'))
    if (Date.now() - Number(j.at ?? 0) < GH_TTL_MS) return j.versions ?? null
  } catch {
    /* 无缓存 / 缓存损坏都当没有 */
  }
  return null
}

function writeCache(repo, versions) {
  try {
    mkdirSync(CACHE_DIR, { recursive: true })
    writeFileSync(cachePath(repo), JSON.stringify({ at: Date.now(), versions }), 'utf8')
  } catch (e) {
    console.error(`[warn] 版本缓存写入失败 ${repo}: ${e.message}`)
  }
}

/** 分页拉全 releases，滤掉服务端产物；返回客户端可下载的资源 */
async function fetchGhVersions(repo) {
  const cached = readCache(repo)
  if (cached) return cached

  const headers = { 'user-agent': 'YYL-Central-Server', accept: 'application/vnd.github+json' }
  if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`

  const out = []
  for (let page = 1; page <= GH_PAGES; page++) {
    const url = `https://api.github.com/repos/${repo}/releases?per_page=100&page=${page}`
    const res = await fetch(url, { headers })
    if (!res.ok) {
      const hint = res.status === 403 ? '（未带 GITHUB_TOKEN 时 60 次/小时，或代理出口 IP 被拦）' : ''
      throw new Error(`GitHub HTTP ${res.status}${hint}`)
    }
    const rels = await res.json()
    if (!Array.isArray(rels)) throw new Error('GitHub 返回的不是数组（可能被镜像/代理拦截）')
    if (!rels.length) break

    for (const r of rels) {
      const assets = []
      let dropped = 0
      for (const a of r.assets ?? []) {
        if (!isClientAsset(a.name)) {
          dropped++
          continue
        }
        assets.push({ name: a.name, url: a.browser_download_url, size: Number(a.size) || 0 })
      }
      // 纯服务端/测试版本：一条客户端资产都不剩，整条隐藏
      if (!assets.length) continue
      out.push({
        tag: String(r.tag_name ?? ''),
        title: clean(r.name, 80) || String(r.tag_name ?? ''),
        date: String(r.published_at ?? '').slice(0, 10),
        pageUrl: String(r.html_url ?? ''),
        assets,
        dropped,
        folder: false,
      })
    }
    if (rels.length < 100) break
  }
  writeCache(repo, out)
  return out
}

/** 直链源只有一个「版本」，就是它自己 */
function directVersions(url) {
  const name = String(url).split('/').filter(Boolean).pop() ?? url
  return [{ tag: name, title: '', date: '', pageUrl: url, assets: [{ name, url, size: 0 }], dropped: 0, folder: false }]
}

/** 逐源标注版本：GitHub 失败不致命，只写 versionsError（客户端据此不再重试网络） */
async function annotateVersions(items) {
  await Promise.all(
    items.map(async (it) => {
      it.versions = []
      if (it.kind === 'direct-url') {
        it.versions = directVersions(it.url)
        return
      }
      // file-list 是 HTML 目录页，没有 API 限额，留给客户端解析
      if (it.kind !== 'github-repo') return
      try {
        it.versions = await fetchGhVersions(it.repo)
        if (!it.versions.length) it.versionsError = 'GitHub 返回 0 条 release'
      } catch (e) {
        it.versionsError = String(e?.message ?? e).slice(0, 160)
        console.error(`[warn] ${it.name} 版本标注失败：${it.versionsError}（客户端将回退本地缓存）`)
      }
    }),
  )
  return items
}

/* ================= 索引生成 ================= */

async function build() {
  const KINDS = ['direct-url', 'github-repo', 'file-list']
  const items = SOURCES.map((s, i) => {
    const kind = KINDS.includes(s.kind) ? s.kind : 'direct-url'
    const ok = kind === 'github-repo'
      ? /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(s.repo ?? '')
      : /^https?:\/\//.test(s.url ?? '')
    if (!ok) throw new Error(`第 ${i + 1} 条源不合法：${s.name}`)
    return {
      id: clean(s.id, 48) || slug(s.name, i),
      name: clean(s.name, 60),
      kind,
      url: clean(s.url, 500),
      repo: clean(s.repo, 120),
      asset: clean(s.asset, 100) || 'Mindustry.jar',
      note: clean(s.note, 80),
      group: clean(s.group, 24) || '中心',
      tags: tagsOf({ ...s, kind }),
      size: Number(s.size) > 0 ? Number(s.size) : 0,
      // logo 由中心服务器下发，用于识别游戏；空则客户端用首字母兜底
      logo: /^https?:\/\//.test(s.logo ?? '') ? clean(s.logo, 500) : '',
      // 标语：一句话说明这个默认游戏端是什么；空则客户端用内置兜底文案
      slogan: clean(s.slogan, 60),
      scope: s.scope === 'server' ? 'server' : 'client',
      versions: [],
      versionsError: '',
    }
  })

  await annotateVersions(items)

  return {
    schema: SCHEMA,
    app: 'yueyue-launcher',
    generated: new Date().toISOString(),
    count: items.length,
    items,
    // 每日信息：Markdown 原文，客户端自己拆行
    infoBar: clean(readInfoBar(), 8000),
    // 镜像清单：只收 http(s) 前缀，名称/地址都裁长度
    mirrors: {
      workshop: (MIRRORS.workshop ?? [])
        .filter((m) => /^https?:\/\//.test(m.url ?? ''))
        .slice(0, 20)
        .map((m) => ({ name: clean(m.name, 40) || clean(m.url, 40), url: clean(m.url, 300) })),
    },
  }
}

async function emit() {
  const idx = await build()
  const json = JSON.stringify(idx, null, 2)
  writeFileSync(OUT, json + '\n', 'utf8')
  return json
}

function summarize(items) {
  for (const it of items) {
    const v = it.versions.length
    const tail = v ? `${v} 个版本` : it.versionsError ? `版本标注失败：${it.versionsError}` : '版本由客户端解析'
    console.log(`  · ${it.name} [${it.tags.join('/')}] slogan=${it.slogan || '—'} → ${tail}`)
  }
}

const CORS = {
  'access-control-allow-origin': '*',
  'cache-control': 'no-store',
}

async function serve(port) {
  console.log(`YYL 中心化服务器启动中，监听 0.0.0.0:${port}`)
  console.log(`  · 中心化索引  GET /index.json`)
  console.log(`  · 信息服务器  GET /info.md   （每日信息 Markdown，可直接填进设置的「每日信息」链接）`)
  console.log(`  · 健康检查    GET /health`)
  console.log('在客户端「设置 → 镜像与网络 → 中心化服务器」填入本服务地址即可')

  const server = createServer((req, res) => {
    const url = (req.url || '/').split('?')[0]
    // 访问日志：便于确认「客户端是开机拉还是进页面才拉」
    console.log(`[${new Date().toISOString()}] ${req.method} ${url} ${req.headers['user-agent'] || ''}`.trim())

    // 信息服务器：把每日信息以 Markdown 原文吐出去，
    // 客户端「每日信息」那一行可以直接填 http(s)://<本服务>/info.md
    if (url === '/info.md' || url === '/info') {
      res.writeHead(200, { 'content-type': 'text/markdown; charset=utf-8', ...CORS })
      res.end(readInfoBar() + '\n')
      return
    }

    if (url === '/health') {
      res.writeHead(200, { 'content-type': 'text/plain; charset=utf-8', ...CORS })
      res.end('ok\n')
      return
    }

    if (url !== '/' && url !== '/index.json') {
      res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8', ...CORS }).end('not found')
      return
    }

    emit().then(
      (body) => {
        res.writeHead(200, { 'content-type': 'application/json; charset=utf-8', ...CORS })
        res.end(body)
      },
      (e) => {
        res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8', ...CORS }).end(String(e))
      },
    )
  })

  // 先监听再建索引：GitHub 索引可能要几十秒，若放在 listen 之前，
  // 容器启动期 /health 会连不上（实测踩过），编排会误判成起不来。
  server.listen(port, '0.0.0.0', () => {
    console.log(`已监听，首次索引在后台构建…`)
    emit().then(
      (body) => {
        const idx = JSON.parse(body)
        console.log(`首次索引完成（schema ${idx.schema}，${idx.count} 条源）`)
        summarize(idx.items)
      },
      (e) => console.error('首次索引失败（收到请求时会自动重试）:', e),
    )
  })
}

const [, , cmd = 'build', arg] = process.argv
if (cmd === 'serve') {
  // 端口优先级：命令行参数 > 环境变量 PORT > 8787（容器里用 PORT 便于 docker-compose 配置）
  await serve(Number(arg) || Number(process.env.PORT) || 8787)
} else {
  const idx = JSON.parse(await emit())
  console.log(`已生成 ${OUT}（schema ${idx.schema}，${idx.count} 条源）`)
  summarize(idx.items)
}
