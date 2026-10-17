import http from 'node:http';
import net from 'node:net';
import { lookup } from 'node:dns/promises';
import ipaddr from 'ipaddr.js';

export function publicAddress(address) {
  try {
    const parsed = ipaddr.process(address);
    return parsed.range() === 'unicast';
  } catch { return false; }
}
export function safeUrl(value) {
  const url = new URL(value);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('invalid_target');
  url.search = ''; url.hash = '';
  return url.href.slice(0, 500);
}
export async function resolveTarget(value, origins, resolver = lookup) {
  const url = new URL(value);
  safeUrl(url.href);
  if (!origins.has(url.origin)) throw new Error('origin_not_allowed');
  const host = url.hostname.replace(/^\[|\]$/g, '');
  const addresses = net.isIP(host) ? [{ address: host }] : await resolver(host, { all: true, verbatim: true });
  if (!addresses.length || addresses.some(row => !publicAddress(row.address))) throw new Error('private_destination');
  return { address: addresses[0].address, port: Number(url.port || (url.protocol === 'https:' ? 443 : 80)), url };
}

/** Every browser connection passes here, including redirects and subresources.
 * DNS answers are checked together, then the TCP socket uses the pinned address. */
export async function startProxy(origins, { maxRequests = 300, maxBytes = 50 * 1024 * 1024, resolver } = {}) {
  const sockets = new Set();
  let requests = 0, bytes = 0, blocked = 0, closed = false;
  const decisions = new Map();
  async function target(raw) {
    if (closed || ++requests > maxRequests) throw new Error('request_budget');
    const origin = new URL(raw).origin;
    if (!decisions.has(origin)) {
      if (decisions.size >= 17) throw new Error('origin_budget');
      decisions.set(origin, resolveTarget(raw, origins, resolver));
    }
    return decisions.get(origin);
  }
  function track(socket) {
    sockets.add(socket);
    socket.setTimeout(20_000, () => socket.destroy());
    socket.on('error', () => {});
    socket.on('close', () => sockets.delete(socket));
    socket.on('data', chunk => {
      bytes += chunk.length;
      if (bytes > maxBytes) { closed = true; for (const item of sockets) item.destroy(); }
    });
    return socket;
  }
  const server = http.createServer(async (req, res) => {
    req.on('error', () => res.destroy());
    res.on('error', () => {});
    try {
      const { address, port } = await target(req.url);
      const url = new URL(req.url);
      if (url.protocol !== 'http:' || !['GET','HEAD','POST','OPTIONS','PUT','PATCH','DELETE'].includes(req.method)) throw new Error('invalid_proxy_request');
      const headers = { ...req.headers, host: url.host };
      delete headers['proxy-authorization']; delete headers['proxy-connection'];
      const upstream = http.request({ host: address, port, method: req.method, path: url.pathname + url.search, headers, timeout: 20_000 }, response => {
        res.writeHead(response.statusCode || 502, response.headers);
        response.on('error', () => res.destroy());
        response.pipe(res);
      });
      upstream.on('socket', track);
      upstream.on('timeout', () => upstream.destroy());
      upstream.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
      req.pipe(upstream);
    } catch { blocked++; res.writeHead(403); res.end('Destination blocked'); }
  });
  // Track client sockets before asynchronous DNS/policy checks can reject them.
  server.on('connection', track);
  server.on('connect', async (req, client, head) => {
    try {
      const { address, port } = await target('https://' + req.url);
      const upstream = track(net.connect({ host: address, port }));
      upstream.once('connect', () => {
        client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
        if (head.length) upstream.write(head);
        client.pipe(upstream); upstream.pipe(client);
      });
      client.on('error', () => upstream.destroy());
      client.on('close', () => upstream.destroy());
      upstream.on('error', () => client.destroy());
    } catch { blocked++; client.end('HTTP/1.1 403 Forbidden\r\n\r\n'); }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    stats: () => ({ requests, bytes, blocked }),
    close: async () => { closed = true; for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); },
  };
}
