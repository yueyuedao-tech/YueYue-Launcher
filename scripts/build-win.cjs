const { build } = require('electron-builder')

build({ win: ['dir'] })
  .then(() => {
    console.log('BUILD_OK')
    const { trim } = require('./trim-unpacked.cjs')
    trim()
  })
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
