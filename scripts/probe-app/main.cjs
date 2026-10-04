const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

setTimeout(() => app.exit(2), 25000)

app.whenReady().then(async () => {
  try {
    const win = new BrowserWindow({ show: false, width: 1280, height: 800 })
    await win.loadFile(path.join(__dirname, '..', '..', 'dist', 'index.html'))
    await new Promise((r) => setTimeout(r, 700))
    const info = await win.webContents.executeJavaScript(`(function () {
      const el = document.querySelector('.home-bg');
      if (!el) return { found: false };
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      return {
        found: true,
        position: cs.position,
        zIndex: cs.zIndex,
        rect: { x: r.x, y: r.y, w: r.width, h: r.height },
        bgImage: cs.backgroundImage.slice(0, 100),
        bgSize: cs.backgroundSize,
        bgPos: cs.backgroundPosition,
        parentPos: getComputedStyle(el.parentElement).position,
        parentCls: el.parentElement.className,
      };
    })()`)
    fs.writeFileSync(path.join(__dirname, 'probe.json'), JSON.stringify(info, null, 2))
    app.exit(0)
  } catch (e) {
    fs.writeFileSync(path.join(__dirname, 'probe.json'), String((e && e.stack) || e))
    app.exit(3)
  }
})
