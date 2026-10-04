const { build } = require('electron-builder')

build({ win: ['dir'] })
  .then(() => console.log('BUILD_OK'))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
