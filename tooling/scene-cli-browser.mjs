import { spawn } from 'node:child_process';
import { once } from 'node:events';
import fs from 'node:fs/promises';
import path from 'node:path';

export const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
export async function until(check, description, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    try { if (await check()) return; } catch {}
    await sleep(100);
  }
  throw new Error(`Timed out: ${description}`);
}

export async function chrome(executable, profile, log) {
  const child = spawn(executable, ['--headless', '--no-sandbox', '--disable-dev-shm-usage',
    '--disable-background-networking', '--no-first-run', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'],
  { stdio: ['ignore', log, log] });
  const exited = once(child, 'exit').catch(() => null);
  await until(async () => {
    if (child.exitCode !== null) throw new Error('Chromium exited');
    return (await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).includes('\n');
  }, 'Chromium DevTools startup').catch(error => { child.kill('SIGTERM'); throw error; });
  const port = (await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0];
  const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await once(ws, 'open');
  const pending = new Map();
  let sequence = 0;
  const errors = [];
  ws.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) {
      const request = pending.get(message.id);
      if (!request) return;
      clearTimeout(request.timer);
      pending.delete(message.id);
      message.error ? request.reject(new Error(message.error.message)) : request.resolve(message.result);
    } else if (message.method === 'Runtime.exceptionThrown') {
      errors.push(message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text);
    } else if (message.method === 'Page.javascriptDialogOpening') {
      dialogs.push({ type: message.params.type, message: message.params.message });
      send('Page.handleJavaScriptDialog', { accept: true }).catch(error => errors.push(error.message));
    } else if (message.method === 'Fetch.requestPaused') {
      const { requestId, request } = message.params;
      const url = new URL(request.url);
      const allowed = ['127.0.0.1', 'localhost'].includes(url.hostname);
      send(allowed ? 'Fetch.continueRequest' : 'Fetch.failRequest', allowed ? { requestId } : { requestId, errorReason: 'BlockedByClient' }).catch(error => errors.push(error.message));
    }
  });
  const dialogs = [];
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP deadline: ${method}`)); }, 15000);
    pending.set(id, { resolve, reject, timer });
    ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', { expression: `(()=>{${expression}})()`, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    return result.result.value;
  };
  await send('Page.enable');
  await send('Runtime.enable');
  await send('Fetch.enable', { patterns: [{ urlPattern: 'http*' }] });
  const size = (width = 1440, height = 1000) => send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  await size();
  return {
    errors, dialogs, send, evaluate, size,
    async go(url) {
      await send('Page.navigate', { url });
      await until(() => evaluate(`return location.href === ${JSON.stringify(url)} && document.readyState === 'complete'`), `navigation to ${url}`);
      await evaluate('return document.fonts.ready.then(()=>true)');
    },
    async screenshot(file) {
      const { cssContentSize } = await send('Page.getLayoutMetrics');
      const result = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true,
        clip: { x: 0, y: 0, width: cssContentSize.width, height: Math.min(cssContentSize.height, 12000), scale: 1 } });
      await fs.writeFile(file, Buffer.from(result.data, 'base64'));
    },
    async close() {
      ws.close();
      child.kill('SIGTERM');
      await Promise.race([exited, sleep(3000)]);
      if (child.exitCode === null) child.kill('SIGKILL');
    },
  };
}
