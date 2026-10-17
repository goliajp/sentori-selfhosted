FROM oven/bun:1.4.3 AS dependencies
WORKDIR /dependencies
COPY self-hosted/web-runner/package.json self-hosted/web-runner/bun.lock ./
RUN bun install --frozen-lockfile --production

FROM node:24-bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends chromium ca-certificates fonts-liberation && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=dependencies /dependencies/node_modules ./self-hosted/web-runner/node_modules
COPY self-hosted/web-runner/*.mjs self-hosted/web-runner/package.json ./self-hosted/web-runner/
COPY scripts/lib/headless-chrome.mjs ./scripts/lib/headless-chrome.mjs
ENV CHROME_PATH=/usr/bin/chromium
USER node
ENTRYPOINT ["node","self-hosted/web-runner/once.mjs"]
