const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

const out = path.join(__dirname, 'smoke-render.json')

function finish(code, data) {
  try { fs.writeFileSync(out, JSON.stringify(data, null, 2)) } catch (e) { /* ignore */ }
  app.exit(code)
}

setTimeout(() => finish(2, { error: 'timeout 20s' }), 20000)

app.whenReady().then(async () => {
  try {
    const win = new BrowserWindow({ show: false })
    await win.loadFile(path.join(__dirname, '..', 'dist', 'index.html'))
    await new Promise((r) => setTimeout(r, 1500))
    const result = await win.webContents.executeJavaScript(`({
      title: document.title,
      appChildren: (document.getElementById('app') || {}).childElementCount || 0,
      text: (document.body.innerText || '').slice(0, 400)
    })`, true)
    finish(result.appChildren > 0 ? 0 : 1, result)
  } catch (e) {
    finish(3, { error: String(e && e.stack || e) })
  }
})
