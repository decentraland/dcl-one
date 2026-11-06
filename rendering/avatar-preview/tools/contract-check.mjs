#!/usr/bin/env node
// Drives the preview page in headless Chrome over the DevTools protocol the way a host would: waits
// for the page's `load` (or `error`) message, posts controller requests through `postMessage`, and
// saves the canvas and the screenshot the page returned. Needs `?debug=1` on the url (the page then
// keeps its host messages in `window.__messages`) and a WebGPU-capable Chrome.
//
//   node tools/contract-check.mjs "<url>" [seconds] [outPrefix]
//
// `CHROME_PATH` names the browser (default: Windows Chrome from WSL). Each console line that
// mentions a panic, an exception, an error or a command is printed at the end.
import { spawn, spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const [url, secondsArg, outArg] = process.argv.slice(2);
if (!url) {
  console.error('usage: contract-check.mjs <url> [seconds] [outPrefix]');
  process.exit(2);
}
const seconds = Number(secondsArg || 30);
const out = outArg || 'contract-check';
const chromePath = process.env.CHROME_PATH || '/mnt/c/Program Files/Google/Chrome/Application/chrome.exe';
const port = Number(process.env.CDP_PORT || 9333);

const chrome = spawn(chromePath, [
  // HEADED=1 opens a real window (what the screen composites), otherwise headless
  ...(process.env.HEADED ? [] : ['--headless=new']),
  '--enable-unsafe-webgpu', '--no-first-run', '--no-default-browser-check',
  '--autoplay-policy=no-user-gesture-required',
  // a test browser never plays sound on the machine it runs on; UNMUTE=1 to listen
  ...(process.env.UNMUTE ? [] : ['--mute-audio']),
  // keep cross-origin iframes in the page's process, so their console reaches this one listener;
  // SITE_ISOLATION=1 keeps Chrome's default process-per-site instead (compositing then matches a
  // headed browser, but an iframe's console is not captured)
  ...(process.env.SITE_ISOLATION ? [] : ['--disable-features=site-per-process']),
  `--user-data-dir=${process.env.CHROME_PROFILE || 'C:\\Users\\Public\\dcl-preview-chrome-profile'}`,
  `--remote-debugging-port=${port}`, `--window-size=${process.env.WINDOW || '512,768'}`,
  // CHROME_EXTRA_ARGS: more flags, space-separated (for example a GPU backend selection)
  ...(process.env.CHROME_EXTRA_ARGS ? process.env.CHROME_EXTRA_ARGS.split(' ').filter(Boolean) : []),
  'about:blank',
], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = process.env.CHROME_PROFILE || 'C:\\Users\\Public\\dcl-preview-chrome-profile';
// Windows Chrome launched from WSL outlives the killed launcher: end every process that uses this
// run's profile (and only those), and wait for it, so no browser keeps running after the check
const finish = (code) => {
  chrome.kill();
  if (process.platform === 'linux' && chromePath.startsWith('/mnt/')) {
    const dir = profile.split('\\').pop();
    const ps = `Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*${dir}*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }`;
    spawnSync('/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe', ['-NoProfile', '-Command', ps], { stdio: 'ignore' });
  }
  process.exit(code);
};
process.on('SIGINT', () => finish(130));
process.on('SIGTERM', () => finish(143));

let targets;
for (let i = 0; i < 40 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); } catch { await sleep(250); }
}
if (!targets) { console.error('no debugger on', port); finish(1); }
const ws = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
const logs = [];
const send = (method, params = {}) => new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d.result); pending.delete(d.id); return; }
  if (d.method === 'Runtime.consoleAPICalled') logs.push(`[${d.params.type}] ${d.params.args.map((a) => a.value ?? a.description ?? '').join(' ').replace(/%c/g, '').replace(/ color: whitesmoke.*$/, '')}`);
  if (d.method === 'Runtime.exceptionThrown') logs.push(`[exception] ${d.params.exceptionDetails.text} ${d.params.exceptionDetails.exception?.description ?? ''}`);
};
await send('Runtime.enable'); await send('Page.enable'); await send('Network.enable');
await send('Network.setCacheDisabled', { cacheDisabled: true });
{ const [w, h] = (process.env.WINDOW || '512,768').split(',').map(Number); await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false }); }
await send('Page.navigate', { url });
// TIMELINE (comma-separated seconds after navigation) captures the page at each, as
// `<outPrefix>-t<seconds>.png`: a host page's playback over time, whether or not it reports `load`
for (const t of (process.env.TIMELINE || '').split(',').filter(Boolean).map(Number)) {
  setTimeout(async () => {
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    if (shot?.data) writeFileSync(`${out}-t${t}.png`, Buffer.from(shot.data, 'base64'));
  }, t * 1000);
}
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.value;

const deadline = Date.now() + seconds * 1000;
let loaded = false;
while (Date.now() < deadline) {
  await sleep(500);
  // a host page without `window.__messages` (a real embedding site) just runs out the clock
  if (await evaluate("Array.isArray(window.__messages) && window.__messages.some(m => m.type === 'load' || m.type === 'error')")) { loaded = true; break; }
}
console.log('LOADED', loaded, 'after', Math.round((seconds * 1000 - (deadline - Date.now())) / 100) / 10, 's');

const requests = [
  { id: 'r1', namespace: 'scene', method: 'getMetrics', params: [] },
  { id: 'r2', namespace: 'emote', method: 'isPlaying', params: [] },
  { id: 'r3', namespace: 'scene', method: 'changeCameraPosition', params: [{ alpha: 0.6 }] },
  { id: 'r4', namespace: 'scene', method: 'getScreenshot', params: [256, 384] },
  { id: 'r5', namespace: 'emote', method: 'goTo', params: [1] },
];
// REQUESTS (json array) replaces the default requests; `{ "wait": ms }` items pause between them
const script = process.env.REQUESTS ? JSON.parse(process.env.REQUESTS) : requests;
for (const r of script) {
  if (r.wait) { await sleep(r.wait); continue; }
  await evaluate(`window.postMessage({ type: 'controller_request', payload: ${JSON.stringify(r)} }, '*')`);
}
await sleep(6000);
const messages = await evaluate("JSON.stringify((window.__messages || []).map(m => m.type === 'controller_response' && m.payload.ok && typeof m.payload.result === 'string' && m.payload.result.startsWith('data:image') ? { ...m, payload: { ...m.payload, result: `data:image… (${m.payload.result.length} chars)` } } : m))");
console.log('MESSAGES', messages);
const shot = await evaluate("((window.__messages || []).find(m => m.type === 'controller_response' && m.payload.id === 'r4') || {}).payload?.result?.split(',')[1] || ''");
if (shot) writeFileSync(`${out}-screenshot.png`, Buffer.from(shot, 'base64'));
const canvas = await evaluate("(() => { const c = document.querySelector('canvas'); return c ? c.toDataURL('image/png').split(',')[1] : ''; })()");
console.log('FRAMES', await evaluate("JSON.stringify([...document.querySelectorAll('iframe')].map(f => { const r = f.getBoundingClientRect(); return { src: f.src.slice(0, 120), w: Math.round(r.width), h: Math.round(r.height), display: getComputedStyle(f).display, visibility: getComputedStyle(f).visibility }; }))"));
if (canvas) writeFileSync(`${out}-canvas.png`, Buffer.from(canvas, 'base64'));
// `PROBE_JS` is evaluated in the page at the end and printed, for a host page's own state; the page
// itself is also captured (`<outPrefix>-page.png`), which shows a host's overlays even where WebGPU
// canvases come out black in headless mode
if (process.env.PROBE_JS) console.log('PROBE', await evaluate(process.env.PROBE_JS));
const page = await send('Page.captureScreenshot', { format: 'png' });
if (page?.data) writeFileSync(`${out}-page.png`, Buffer.from(page.data, 'base64'));
console.log(logs.filter((l) => /panic|exception|ERROR|WARN|->|<-|emote "/.test(l)).join('\n'));
ws.close();
finish(0);
