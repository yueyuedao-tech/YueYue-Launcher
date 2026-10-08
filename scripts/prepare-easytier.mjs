import { execFileSync } from 'node:child_process'
import { createWriteStream, existsSync, mkdirSync, rmSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'

const version = 'v2.6.4'
const platform = process.platform === 'win32' ? 'windows-x86_64' : process.platform === 'linux' ? 'linux-x86_64' : ''
if (!platform || process.arch !== 'x64') throw new Error('EasyTier bundle currently supports Windows/Linux x64')
const file = `easytier-${platform}-${version}.zip`
const url = `https://github.com/EasyTier/EasyTier/releases/download/${version}/${file}`
const root = resolve('src-tauri', 'resources', 'easytier')
const core = join(root, process.platform === 'win32' ? 'easytier-core.exe' : 'easytier-core')
const zip = join(root, file)
mkdirSync(root, { recursive: true })
if (!existsSync(core)) {
  const response = await fetch(url)
  if (!response.ok || !response.body) throw new Error(`EasyTier download failed: ${response.status}`)
  await pipeline(Readable.fromWeb(response.body), createWriteStream(zip))
  const entry = `easytier-${platform}/${process.platform === 'win32' ? 'easytier-core.exe' : 'easytier-core'}`
  execFileSync('tar', ['-xf', zip, '-C', root, entry])
  const source = join(root, entry)
  const { renameSync, chmodSync } = await import('node:fs')
  renameSync(source, core)
  if (process.platform !== 'win32') chmodSync(core, 0o755)
  if (process.platform === 'win32') {
    execFileSync('tar', ['-xf', zip, '-C', root, `easytier-${platform}/wintun.dll`])
    renameSync(join(root, `easytier-${platform}`, 'wintun.dll'), join(root, 'wintun.dll'))
  }
  rmSync(join(root, `easytier-${platform}`), { recursive: true, force: true })
  rmSync(zip)
}
console.log(`EasyTier ${version}: ${core}`)
