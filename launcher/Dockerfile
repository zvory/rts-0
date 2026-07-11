FROM node:22-alpine

WORKDIR /app
COPY launcher/server.mjs launcher/index.html ./

ENV NODE_ENV=production
EXPOSE 8080
CMD ["node", "server.mjs"]
