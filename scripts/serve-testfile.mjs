/**
 * 本地测速用文件服务器：支持 Range（默认）与「不支持 Range」两种模式。
 * 用法: node scripts/serve-testfile.mjs [端口] [MB] [--norange] [--rate=KB/s]
 *   --rate 为**每条连接**的限速（KB/s），用来把传输拖慢到能观察速度/进度的程度。
 * 用途: 让启动器的多线程下载走一个可控、可复现的目标（公网 GitHub 入口在本机不稳定）
 */
import { createServer } from 'node:http'
import { createHash } from 'node:crypto'

const port = Number(process.argv[2]) || 8899
const mb = Number(process.argv[3]) || 40
const noRange = process.argv.includes('--norange')
const rateArg = process.argv.find((a) => a.startsWith('--rate='))
const rateKB = rateArg ? Number(rateArg.split('=')[1]) : 0

const SIZE = mb * 1024 * 1024
const body = Buffer.alloc(SIZE)
for (let i = 0; i < SIZE; i++) body[i] = (i * 131 + (i >> 9) * 17) & 0xff
const digest = createHash('sha256').update(body).digest('hex')

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

/** 限速分块写；rateKB=0 时一次性写完 */
async function send(res, buf) {
  if (!rateKB) {
    res.end(buf)
    return
  }
  const step = 32 * 1024
  const delayMs = Math.round((step / 1024 / rateKB) * 1000)
  for (let off = 0; off < buf.length; off += step) {
    res.write(buf.subarray(off, off + step))
    if (delayMs > 0) await sleep(delayMs)
  }
  res.end()
}

const server = createServer(async (req, res) => {
  if (req.url && req.url.startsWith('/sha256')) {
    res.writeHead(200, { 'content-type': 'text/plain' })
    res.end(digest)
    return
  }
  const range = req.headers.range
  if (noRange || !range) {
    res.writeHead(200, { 'content-length': SIZE, 'content-type': 'application/java-archive' })
    await send(res, body)
    return
  }
  const m = /bytes=(\d+)-(\d*)/.exec(range)
  const start = Number(m[1])
  const end = m[2] ? Math.min(Number(m[2]), SIZE - 1) : SIZE - 1
  const chunk = body.subarray(start, end + 1)
  res.writeHead(206, {
    'content-length': chunk.length,
    'content-range': `bytes ${start}-${end}/${SIZE}`,
    'accept-ranges': 'bytes',
  })
  await send(res, chunk)
})

server.listen(port, '127.0.0.1', () => {
  console.log(
    `测试文件服务: http://127.0.0.1:${port}/big.jar  (${mb}MB, ${noRange ? '不支持 Range' : '支持 Range'}` +
      `${rateKB ? `, 限速 ${rateKB}KB/s/连接` : ''})`,
  )
  console.log(`sha256=${digest}`)
  console.log(`sha256 查询: http://127.0.0.1:${port}/sha256`)
})
