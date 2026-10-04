const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

setTimeout(() => app.exit(2), 35000)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const outDir = path.join(__dirname, 'out')
const report = []

app.whenReady().then(async () => {
  try {
    fs.mkdirSync(outDir, { recursive: true })

    // 1) 移动端布局
    const mobile = new BrowserWindow({ show: false, width: 400, height: 780 })
    await mobile.loadFile(path.join(__dirname, '..', '..', 'dist', 'index.html'))
    await sleep(700)
    fs.writeFileSync(path.join(outDir, 'mobile-home.png'), (await mobile.webContents.capturePage()).toPNG())
    const railBox = await mobile.webContents.executeJavaScript(`(function () {
      const r = document.querySelector('.rail').getBoundingClientRect();
      return { x: r.x, y: r.y, w: r.width, h: r.height };
    })()`)
    report.push('mobile rail: ' + JSON.stringify(railBox))
    await mobile.webContents.executeJavaScript(`document.querySelectorAll('.rail-btn')[3].click(); true`)
    await sleep(400)
    fs.writeFileSync(path.join(outDir, 'mobile-settings.png'), (await mobile.webContents.capturePage()).toPNG())
    mobile.destroy()

    // 2) 设置持久化：改内存滑条 -> 重载 -> 读 localStorage
    const win = new BrowserWindow({ show: false, width: 1280, height: 800 })
    await win.loadFile(path.join(__dirname, '..', '..', 'dist', 'index.html'))
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
    await sleep(300)
    const before = await win.webContents.executeJavaScript(`localStorage.getItem('starlight-launcher-settings')`)
    report.push('before reload: ' + before)
    await win.webContents.loadFile(path.join(__dirname, '..', '..', 'dist', 'index.html'))
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

    fs.writeFileSync(path.join(outDir, 'report.txt'), report.join('\n'))
    app.exit(0)
  } catch (e) {
    fs.writeFileSync(path.join(outDir, 'report.txt'), report.join('\n') + '\nERR ' + ((e && e.stack) || e))
    app.exit(3)
  }
})
