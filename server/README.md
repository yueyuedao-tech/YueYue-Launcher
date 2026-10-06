# YYL 中心化服务器 / 信息服务器

一个**零依赖**的 Node 脚本，同时扮演两个角色：

| 角色 | 地址 | 内容 |
|---|---|---|
| 中心化服务器 | `/index.json` | 源清单 + logo/标语/标签 + 版本快照 + 镜像清单 + 每日信息 |
| 信息服务器 | `/info.md` | 每日信息 Markdown 原文（可直接填进客户端的「每日信息」链接） |
| 健康检查 | `/health` | `ok`，供容器编排探活 |

客户端「设置 → 镜像与网络」里：
- **中心化服务器** 填 `https://mindustry.wiki:1020`
- **每日信息** 链接填 `https://mindustry.wiki:1020/info.md`

---

## 目录内容

```
server/
├── central.mjs                 服务端本体（源的配置就写在这里的 SOURCES / MIRRORS / INFO_BAR）
├── index.json                  build 产物：已标注好的索引快照
├── Dockerfile                  镜像定义（node:22-alpine，非 root 运行，自带健康检查）
├── docker-compose.yml          单服务编排，监听 1020
├── docker-compose.tls.yml      可选：额外起一个 Caddy 在 1020 上终止 TLS
├── Caddyfile.example           上面那个 Caddy 的配置
├── nginx.example.conf          已有 nginx/证书时用它加一个 server 块
└── .cache/                     GitHub 版本缓存（15 分钟 TTL，容器里挂到 data/）
```

## 快速部署（Docker）

```bash
cd server

# 1) 先本地生成一次 index.json（可选，镜像里也用得上）
node central.mjs build

# 2) 起服务
docker compose up -d

# 3) 验证
curl -s http://127.0.0.1:1020/health          # ok
curl -s http://127.0.0.1:1020/info.md         # 每日信息 Markdown
curl -s http://127.0.0.1:1020/index.json | head -c 200
```

日志：`docker compose logs -f`（每次请求一行，能看到客户端是开机拉还是进页面才拉）。

## 让它对外是 `https://mindustry.wiki:1020`

容器只跑 HTTP，TLS 有三种做法，按你服务器现状选一种：

### A. 已经有 nginx 和 mindustry.wiki 的证书（最常见）
把 `nginx.example.conf` 里的 server 块加到现有配置，`nginx -s reload`。
证书路径换成你自己的即可。

### B. 没有反向代理，想让 Caddy 自动申请证书
```bash
docker compose -f docker-compose.yml -f docker-compose.tls.yml up -d
```
需要 **80 或 443 能通到这台机器**（ACME 校验）。Caddy 会自动申请并续期
mindustry.wiki 的证书，然后在 1020 上提供 HTTPS。

### C. 用已有的证书文件手动起 TLS
把证书挂进容器，用一个只做 `ssl terminate` 的 nginx 反代（同 A）。

> 注意：`https://mindustry.wiki:1020` 里的 1020 是**对外端口**；
> 容器内部端口也用 1020（`PORT=1020`），映射写成 `1020:1020`，避免两处对不上。

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `PORT` | `8787`（compose 里设成 `1020`） | 监听端口 |
| `YYL_GH_TTL_MS` | `900000`（15 分钟） | GitHub 版本缓存时长，`0` = 每次重拉 |
| `GITHUB_TOKEN` | 空 | 提高 GitHub 限额（不带 token 只有 60 次/小时） |
| `NODE_USE_ENV_PROXY=1` | 空 | 让 Node 的 fetch 走 `HTTPS_PROXY`/`HTTP_PROXY`（Node 24+） |

## 改配置

### 公告 / 每日信息（不用重建镜像）

内容放在 **`content/info.md`**，compose 已经把它挂进容器：

```bash
vi /server/server/yyl-central/content/info.md     # 或者用 1Panel 的文件管理器
curl -s http://127.0.0.1:1019/info.md             # 立刻就能看到新内容
```

- 格式是 Markdown，**一行一条**；行首 `**日期 · 标签**` 之后是标题
- 服务端**每次请求都重新读这个文件**，改完下一次请求就生效，**不用重建镜像、不用重启容器**
- `/index.json` 里的 `infoBar` 与 `/info.md` 用的是同一份内容
- 文件不存在或为空时，回落到 `central.mjs` 里的内置默认公告

### 源清单 / 标语 / logo / 镜像

在 `central.mjs` 顶部：`SOURCES`、`MIRRORS`（`INFO_BAR` 只是公告的内置兜底）。改完：

```bash
docker compose up -d --build     # 重建镜像并重启
```

`index.json` 每次请求（受 TTL 限制）都会重新生成，所以改完代码重建即可生效。

## 端口

对外端口由同目录 **`.env`** 里的 `YYL_PORT` 决定（默认 1020）：

```bash
echo 'YYL_PORT=1019' > .env
docker compose up -d
```

容器内部固定监听 1020；反向代理指向 `127.0.0.1:$YYL_PORT` 即可。

## 不用 Docker 也能跑

```bash
node central.mjs build          # 只生成 index.json（可丢到任意静态托管）
node central.mjs serve 1020     # 起服务
```

静态托管的场景：把 `index.json` 传上去即可当中心化服务器用；
`/info.md` 这一角色可以把 `INFO_BAR` 的内容手工存成一个 `.md` 文件一起放上去。

## 反向代理注意

- 不要缓存 `/index.json`（服务端已带 `cache-control: no-store`），否则客户端会拿到旧版本列表。
- `/info.md` 也是 `no-store`，方便随时改公告。
- 已经开了 `access-control-allow-origin: *`，浏览器里直接 fetch 也能用。
