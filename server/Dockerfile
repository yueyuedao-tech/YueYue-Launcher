# YYL 中心化服务器 / 信息服务器
#
# 零依赖 Node 脚本，本身不需要 npm install；镜像里只放代码 + 已生成的 index.json。
# 构建：  docker build -t yyl-central ./server
# 运行：  docker run -d --name yyl-central -p 1020:1020 -e PORT=1020 yyl-central
# 或直接用同目录的 docker-compose.yml
FROM node:22-alpine

# 时区/编码：日志里是 ISO 时间，这里只影响容器内 date，设成上海便于对日志
ENV TZ=Asia/Shanghai \
    PORT=1020 \
    NODE_ENV=production

WORKDIR /app

# 只拷运行需要的几个东西：服务端脚本、构建产物、以及可编辑内容目录
COPY central.mjs ./central.mjs
COPY index.json ./index.json
COPY content ./content
RUN mkdir -p /app/.cache

# 非 root 运行（node 镜像自带 node 用户）
RUN chown -R node:node /app
USER node

EXPOSE 1020

# 健康检查打到 /health，容器编排里可直接看状态
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||1020)+'/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

# serve 会在收到请求时重新构建索引（GitHub 版本走 15 分钟 TTL 缓存）
CMD ["node", "central.mjs", "serve"]
