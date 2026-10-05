#!/usr/bin/env node
/**
 * YYL 中心化服务器 —— 极简、零依赖
 *
 * 职责：索引源内容 → 按「类型 / 分组 / 附加标签」区分打标 → 输出 index.json
 * 用法：
 *   node server/central.mjs build            生成 server/index.json
 *   node server/central.mjs serve [端口]     起服务（默认 8787），每次请求前重新生成
 *
 * 客户端把「设置 → 下载 → 中心化服务器」填成 http://127.0.0.1:8787 即可。
 * 部署到任意静态托管时，只要能访问到 index.json 就行。
 */
import { createServer } from 'node:http'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const OUT = join(HERE, 'index.json')
const SCHEMA = 1

/** 源内容清单：只写事实，标签由 build 阶段统一派生 */
const SOURCES = [
  {
    name: 'GitHub 官方仓库',
    kind: 'github-repo',
    repo: 'Anuken/Mindustry',
    asset: 'Mindustry.jar',
    group: '官方源',
    note: '官方 releases 索引（Atom），可展开历史版本',
    tags: ['版本索引'],
  },
]

const KIND_TAG = { 'direct-url': '直链', 'github-repo': '仓库' }

const clean = (s, n) => String(s ?? '').trim().slice(0, n)
const slug = (s, i) =>
  clean(s, 48).toLowerCase().replace(/[^a-z0-9一-龥]+/g, '-').replace(/^-|-$/g, '') || `item-${i}`

/** 区分打标签：类型标签 + 分组标签 + 条目自带标签，去重后封顶 8 个 */
function tagsOf(src) {
  const list = [KIND_TAG[src.kind], src.group, ...(src.tags ?? [])]
  return [...new Set(list.filter(Boolean).map((t) => clean(t, 16)))].slice(0, 8)
}

function build() {
  const items = SOURCES.map((s, i) => {
    const kind = s.kind === 'github-repo' ? 'github-repo' : 'direct-url'
    const ok = kind === 'github-repo' ? /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(s.repo ?? '') : /^https?:\/\//.test(s.url ?? '')
    if (!ok) throw new Error(`第 ${i + 1} 条源不合法：${s.name}`)
    return {
      id: slug(s.name, i),
      name: clean(s.name, 60),
      kind,
      url: clean(s.url, 500),
      repo: clean(s.repo, 120),
      asset: clean(s.asset, 100) || 'Mindustry.jar',
      note: clean(s.note, 80),
      group: clean(s.group, 24) || '默认',
      tags: tagsOf({ ...s, kind }),
      size: Number(s.size) > 0 ? Number(s.size) : 0,
    }
  })
  return {
    schema: SCHEMA,
    app: 'yueyue-launcher',
    generated: new Date().toISOString(),
    count: items.length,
    items,
  }
}

function emit() {
  const json = JSON.stringify(build(), null, 2)
  writeFileSync(OUT, json + '\n', 'utf8')
  return json
}

function serve(port) {
  emit()
  const server = createServer((req, res) => {
    const url = (req.url || '/').split('?')[0]
    if (url !== '/' && url !== '/index.json') {
      res.writeHead(404).end('not found')
      return
    }
    let body
    try {
      body = emit() // 每次请求重生成，改完源清单立刻生效
    } catch (e) {
      res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' }).end(String(e))
      return
    }
    res.writeHead(200, {
      'content-type': 'application/json; charset=utf-8',
      'access-control-allow-origin': '*',
      'cache-control': 'no-store',
    })
    res.end(body)
  })
  server.listen(port, '0.0.0.0', () => {
    console.log(`YYL 中心化服务器已启动: http://127.0.0.1:${port}/index.json`)
    console.log('在客户端「设置 → 下载 → 中心化服务器」填入该地址即可')
  })
}

const [, , cmd = 'build', arg] = process.argv
if (cmd === 'serve') {
  serve(Number(arg) || 8787)
} else {
  emit()
  console.log(`已生成 ${OUT}`)
}
