const http = require('http')
const fs = require('fs')
const path = require('path')
const root = path.join(__dirname, '..', 'dist')
http
  .createServer((req, res) => {
    fs.readFile(path.join(root, 'index.html'), (err, data) => {
      if (err) {
        res.writeHead(500)
        res.end('error')
      } else {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end(data)
      }
    })
  })
  .listen(8642, '127.0.0.1', () => console.log('ready on 8642'))
