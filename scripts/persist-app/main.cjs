const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

setTimeout(() => app.exit(2), 30000)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const report = []

app.whenReady().then(async () => {
  try {
    const file = path.join(__dirname, '..', '..', 'dist', 'index.html')
    const win = new BrowserWindow({ show: false, width: 1280, height: 800 })
    await win.loadFile(file)
    await sleep(600)

    await win.webContents.executeJavaScript(`document.querySelectorAll('.rail-btn')[3].click(); true`)
    await sleep(300)
    const changed = await win.webContents.executeJavaScript(`(function () {
      const inp = document.querySelector('input[type=range]');
      if (!inp) return 'no slider';
      inp.value = '8192';
      inp.dispatchEvent(new Event('input', { bubbles: true }));
      return 'set';
    })()`)
    report.push('slider: ' + changed)
    await sleep(400)
    const before = await win.webContents.executeJavaScript(`localStorage.getItem('starlight-launcher-settings')`)
    report.push('before reload: ' + before)

    const loaded = new Promise((resolve) => win.webContents.once('did-finish-load', resolve))
    await win.webContents.executeJavaScript(`location.reload(); true`)
    await loaded
    await sleep(600)
    await win.webContents.executeJavaScript(`document.querySelectorAll('.rail-btn')[3].click(); true`)
    await sleep(300)
    const afterVal = await win.webContents.executeJavaScript(`(function () {
      const inp = document.querySelector('input[type=range]');
      return inp ? inp.value : 'no slider';
    })()`)
    const after = await win.webContents.executeJavaScript(`localStorage.getItem('starlight-launcher-settings')`)
    report.push('after reload slider: ' + afterVal)
    report.push('after reload storage: ' + after)

    fs.writeFileSync(path.join(__dirname, 'report.txt'), report.join('\n'))
    app.exit(0)
  } catch (e) {
    fs.writeFileSync(path.join(__dirname, 'report.txt'), report.join('\n') + '\nERR ' + ((e && e.stack) || e))
    app.exit(3)
  }
})
