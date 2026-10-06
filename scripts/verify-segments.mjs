/**
 * 分段下载算法验证（不依赖应用）：
 * 起一个支持 Range 的本地静态服务 → 用与 Rust 侧完全相同的 curl 参数切段下载 → 按序合并 → 比对 sha256。
 * 这样在编译 3 分钟之前就能确认「切段 + -r + 合并」这条链本身是对的。
 */
import { createServer } from 'node:http'
import { createHash } from 'node:crypto'
import { execFile } from 'node:child_process'
import { mkdtempSync, readFileSync, writeFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'

const run = promisify(execFile)
const SIZE = 5 * 1024 * 1024 + 12345 // 刻意不是整数段，覆盖余数
// 用可校验的伪随机内容：同一字节位置必须稳定
const body = Buffer.alloc(SIZE)
for (let i = 0; i < SIZE; i++) body[i] = (i * 31 + (i >> 8) * 7) & 0xff

const sha = (b) => createHash('sha256').update(b).digest('hex')
const want = sha(body)

const srv = createServer((req, res) => {
  const range = req.headers.range
  if (!range) {
    res.writeHead(200, { 'content-length': SIZE, 'accept-ranges': 'bytes' })
    res.end(body)
    return
  }
  const m = /bytes=(\d+)-(\d*)/.exec(range)
  const start = Number(m[1])
  const end = m[2] ? Number(m[2]) : SIZE - 1
  const chunk = body.subarray(start, end + 1)
  res.writeHead(206, {
    'content-length': chunk.length,
    'content-range': `bytes ${start}-${end}/${SIZE}`,
    'accept-ranges': 'bytes',
  })
  res.end(chunk)
})
await new Promise((r) => srv.listen(0, '127.0.0.1', r))
const url = `http://127.0.0.1:${srv.address().port}/blob`

const dir = mkdtempSync(join(tmpdir(), 'yyl-seg-'))
const N = 4

// ---- 1) 探测：Range 0-0 + --max-filesize 1，必须拿到 206 与总长 ----
const { stdout: head } = await run('curl', ['-sL', '-r', '0-0', '--max-filesize', '1', '-D', '-', '-o', process.platform === 'win32' ? 'NUL' : '/dev/null', url])
const rangeLine = /content-range:\s*bytes 0-0\/(\d+)/i.exec(head)
console.log('探测: 206 =', /HTTP\/[\d.]+ 206/.test(head), '| 总长 =', rangeLine ? rangeLine[1] : '(未拿到)')
if (!rangeLine || Number(rangeLine[1]) !== SIZE) throw new Error('探测失败')

// ---- 2) 切段（与 Rust split_ranges 同规则：余数全给最后一段）----
const chunk = Math.floor(SIZE / N)
const segs = Array.from({ length: N }, (_, i) => [
  i * chunk,
  i === N - 1 ? SIZE - 1 : (i + 1) * chunk - 1,
])
console.log('切段:', segs.map(([s, e]) => `${s}-${e}(${e - s + 1}B)`).join(' '))

// ---- 3) 并发下载各段（与 Rust 侧同参数）----
const parts = segs.map((_, i) => join(dir, `blob.part${i}`))
await Promise.all(
  segs.map(([s, e], i) =>
    run('curl', ['-L', '--fail', '--retry', '2', '-sS', '-o', parts[i], '-r', `${s}-${e}`, url]),
  ),
)
console.log('各段落盘:', parts.map((p) => statSync(p).size).join(' '))

// ---- 4) 合并 + 校验 ----
const merged = Buffer.concat(parts.map((p) => readFileSync(p)))
const got = sha(merged)
console.log('合并大小:', merged.length, '/', SIZE)
console.log('sha256 期望:', want)
console.log('sha256 实得:', got)
console.log(got === want && merged.length === SIZE ? 'PASS 分段下载与单连接结果完全一致' : 'FAIL 内容不一致')

rmSync(dir, { recursive: true, force: true })
srv.close()
process.exit(got === want ? 0 : 1)
