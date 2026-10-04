const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

const outDir = path.join(__dirname, 'shots')
const logPath = path.join(outDir, 'steps.log')
const log = []

function write(note) {
  log.push(note)
  try { fs.writeFileSync(logPath, log.join('\n')) } catch (e) { /* ignore */ }
}

function finish(code, note) {
  write(note)
  try { fs.writeFileSync(path.join(outDir, 'result.txt'), String(note)) } catch (e) { /* ignore */ }
  app.exit(code)
}

setTimeout(() => finish(2, 'timeout'), 40000)

function sleep(ms) { return new Promise((r) => setTimeout(r, ms)) }

app.whenReady().then(async () => {
  try {
    fs.mkdirSync(outDir, { recursive: true })
    const win = new BrowserWindow({ show: false, width: 1280, height: 800 })
    win.webContents.on('console-message', (_e, _l, msg) => write('[console] ' + msg))
    await win.loadFile(path.join(__dirname, '..', '..', 'dist', 'index.html'))
    await sleep(800)
    write('loaded')

    const names = ['home', 'versions', 'downloads', 'settings']
    for (let i = 0; i < names.length; i++) {
      const clickResult = await win.webContents.executeJavaScript(`
        (function () {
          try {
            const btns = document.querySelectorAll('.rail-btn');
            if (btns[${i}]) btns[${i}].click();
            return 'clicked ' + btns.length;
          } catch (e) { return 'CLICK_ERR ' + e.message; }
        })()
      `)
      write(names[i] + ': ' + clickResult)
      await sleep(500)
      const img = await win.webContents.capturePage()
      fs.writeFileSync(path.join(outDir, names[i] + '.png'), img.toPNG())
      write(names[i] + ': captured')
    }

    const homeResult = await win.webContents.executeJavaScript(`
      (function () {
        try {
          const b0 = document.querySelectorAll('.rail-btn')[0];
          if (b0) b0.click();
          return 'home ok';
        } catch (e) { return 'ERR ' + e.message; }
      })()
    `)
    write('back home: ' + homeResult)
    await sleep(400)

    const launchResult = await win.webContents.executeJavaScript(`
      (function () {
        try {
          const b = Array.from(document.querySelectorAll('button')).find(x => x.classList.contains('btn-grad'));
          if (b) b.click();
          return b ? 'launch clicked' : 'launch button missing';
        } catch (e) { return 'ERR ' + e.message; }
      })()
    `)
    write('launch: ' + launchResult)
    await sleep(2600)
    const img = await win.webContents.capturePage()
    fs.writeFileSync(path.join(outDir, 'launch.png'), img.toPNG())
    write('launch: captured')

    finish(0, 'ok')
  } catch (e) {
    finish(3, String((e && e.stack) || e))
  }
})
