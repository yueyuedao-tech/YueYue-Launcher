import { readFileSync } from 'node:fs'

const html = readFileSync('dist/index.html', 'utf8')
const problems = []

for (const m of html.matchAll(/<script[^>]*\bsrc=["']([^"']+)["']/gi)) {
  if (!m[1].startsWith('data:')) problems.push(`external/rel script src: ${m[1]}`)
}
for (const m of html.matchAll(/<link[^>]*\bhref=["']([^"']+)["']/gi)) {
  if (!m[1].startsWith('data:')) problems.push(`external/rel link href: ${m[1]}`)
}
for (const m of html.matchAll(/url\(\s*["']?(https?:\/\/[^)"']+)/gi)) {
  problems.push(`remote css url: ${m[1]}`)
}

if (problems.length) {
  console.error('FAIL: dist/index.html is not self-contained')
  for (const p of problems) console.error('  - ' + p)
  process.exit(1)
}
console.log(`PASS: dist/index.html self-contained (${html.length} bytes)`)
