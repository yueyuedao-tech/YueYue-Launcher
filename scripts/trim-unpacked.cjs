/**
 * 打包产物裁剪：把用不到的文件从 release/win-unpacked 移到 release/trimmed-backup/
 * （不直接删除，便于随时"缝回去"）。
 *
 * 恢复（缝上）方式二选一：
 *   1) node scripts/trim-unpacked.cjs restore   —— 把 backup 里的文件原样拷回
 *   2) SKIP_TRIM=1 重新构建                     —— 构建时跳过裁剪
 * 另外 locales 精简由 electron-builder.yml 的 electronLanguages 控制，
 * 想多带语言直接往那个列表里加代码即可。
 */
const fs = require('fs')
const path = require('path')

const root = path.join(__dirname, '..', 'release', 'win-unpacked')
const backup = path.join(__dirname, '..', 'release', 'trimmed-backup')

// 需要时从备份拷回即可；这三项仅影响 ANGLE 的 Vulkan 后端，Windows 下自动回退 D3D11
const TRIM_FILES = ['vk_swiftshader.dll', 'vulkan-1.dll', 'vk_swiftshader_icd.json']

// 兜底：若 electronLanguages 未生效，locales 里只保留这些
const KEEP_LOCALES = ['zh-CN.pak', 'en-US.pak', 'en-GB.pak']

function dirSize(p) {
  const st = fs.statSync(p)
  if (st.isFile()) return st.size
  let sum = 0
  for (const e of fs.readdirSync(p, { withFileTypes: true })) {
    sum += dirSize(path.join(p, e.name))
  }
  return sum
}

function trim() {
  if (!fs.existsSync(root)) {
    console.log('[trim] win-unpacked 不存在，跳过')
    return 0
  }
  if (process.env.SKIP_TRIM === '1') {
    console.log('[trim] SKIP_TRIM=1，跳过裁剪')
    return 0
  }
  fs.mkdirSync(backup, { recursive: true })
  let saved = 0

  const move = (rel) => {
    const src = path.join(root, rel)
    if (!fs.existsSync(src)) return
    const dst = path.join(backup, rel)
    fs.mkdirSync(path.dirname(dst), { recursive: true })
    if (fs.existsSync(dst)) fs.rmSync(dst, { recursive: true, force: true })
    const size = dirSize(src)
    fs.renameSync(src, dst)
    saved += size
    console.log(`[trim] 移出 ${rel} (${(size / 1024 / 1024).toFixed(2)} MB)`)
  }

  for (const f of TRIM_FILES) move(f)

  const localesDir = path.join(root, 'locales')
  if (fs.existsSync(localesDir)) {
    for (const f of fs.readdirSync(localesDir)) {
      if (!KEEP_LOCALES.includes(f)) move(path.join('locales', f))
    }
  }

  console.log(`[trim] 合计移出 ${(saved / 1024 / 1024).toFixed(1)} MB → release/trimmed-backup/`)
  return saved
}

function restore() {
  if (!fs.existsSync(backup)) {
    console.log('[restore] 无备份，无需恢复')
    return
  }
  const copyBack = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const src = path.join(dir, e.name)
      const rel = path.relative(backup, src)
      const dst = path.join(root, rel)
      if (e.isDirectory()) {
        copyBack(src)
      } else {
        fs.mkdirSync(path.dirname(dst), { recursive: true })
        fs.copyFileSync(src, dst)
        console.log(`[restore] 还原 ${rel}`)
      }
    }
  }
  copyBack(backup)
  console.log('[restore] 完成，备份保留在 release/trimmed-backup/')
}

if (require.main === module) {
  if (process.argv[2] === 'restore') restore()
  else trim()
}
module.exports = { trim, restore }
