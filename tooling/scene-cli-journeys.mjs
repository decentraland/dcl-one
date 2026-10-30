import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import fs from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { chrome, until, sleep } from './scene-cli-browser.mjs';

const { values } = parseArgs({ options: {
  binary: { type: 'string' }, output: { type: 'string' }, revision: { type: 'string' },
  label: { type: 'string', default: 'head' }, chrome: { type: 'string', default: process.env.CHROME_BIN || 'google-chrome' },
} });
if (!values.binary || !values.output || !/^[a-z0-9-]+$/.test(values.label)) throw new Error('--binary, --output and a safe --label are required');
const binary = path.resolve(values.binary);
const output = path.resolve(values.output);
await fs.mkdir(output, { recursive: true });
const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'scene-cli-journeys-'));
const scene = path.join(temporary, 'scene');
const origin = 'http://127.0.0.1:18765';
const upstream = 'http://127.0.0.1:18766';
const manifest = { schema: 1, label: values.label, revision: values.revision || '', stages: [], errors: [], boundaries: [
  'Wallet signatures, live deployment, World mutations and cloud storage are not performed.',
  'Desktop, phone app, hosted player, Inspector, Creator Hub, assistant/MCP and voice runtimes are external boundaries.',
] };
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
  ['PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'LD_LIBRARY_PATH', 'SSL_CERT_FILE', 'NIX_SSL_CERT_FILE'].includes(key)));
Object.assign(env, { DCL_SCENE_CLI_CATALYST: upstream, DCL_SCENE_CLI_CATALYST_ROTATION: upstream,
  DCL_SCENE_CLI_TARGET_SERVER: upstream + '/content', DCL_SCENE_CLI_WORLD_BASE: upstream,
  DCL_SCENE_CLI_FEATURE_FLAGS: upstream, DCL_SCENE_CLI_SOCIAL_API: upstream,
  XDG_CONFIG_HOME: path.join(temporary, 'config'), XDG_CACHE_HOME: path.join(temporary, 'cache') });
let server, browser, preview;
const log = await fs.open(path.join(output, 'preview.log'), 'w');
const browserLog = await fs.open(path.join(output, 'browser.log'), 'w');
const requests = [];
const save = () => fs.writeFile(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2));
try {
  server = http.createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    const url = new URL(request.url, upstream);
    requests.push({ method: request.method, path: url.pathname });
    let body = {};
    if (url.pathname === '/about') body = { healthy: true, content: { publicUrl: upstream + '/content' }, lambdas: { publicUrl: upstream + '/lambdas' } };
    else if (url.pathname.endsWith('/available-content')) body = url.searchParams.getAll('cid').map(cid => ({ cid, available: false }));
    else if (url.pathname.endsWith('/entities/active')) body = [];
    else if (url.pathname.includes('/status')) body = { version: 'journey-fixture', currentTime: 0, worlds: 0 };
    else if (url.pathname.endsWith('/worlds')) body = { data: [], total: 0 };
    else { response.statusCode = 404; body = { message: 'Not configured in the local journey fixture' }; }
    response.setHeader('content-type', 'application/json');
    response.end(JSON.stringify(body));
  });
  server.listen(18766, '127.0.0.1');
  await once(server, 'listening');
  const init = spawnSync(binary, ['init', '--dir', scene, '--project', 'scene', '--yes'], { env, encoding: 'utf8', timeout: 30000 });
  await fs.writeFile(path.join(output, 'init.log'), (init.stdout || '') + (init.stderr || ''));
  if (init.status !== 0) throw new Error('Scene scaffold failed; see init.log');
  const doc = JSON.parse(await fs.readFile(path.join(scene, 'scene.json'), 'utf8'));
  doc.display.title = 'Journey fixture';
  doc.display.description = 'Disposable SDK7 scene for browser journeys.';
  await fs.writeFile(path.join(scene, 'scene.json'), JSON.stringify(doc, null, 2));
  preview = spawn(binary, ['start', '--dir', scene, '--port', '18765', '--no-livekit', '--no-asset-bundles', '--no-mcp', '--no-host', '--data-layer', '--mobile'], { env, stdio: ['ignore', log.fd, log.fd] });
  preview.on('error', error => manifest.errors.push(error.message));
  await until(async () => (await fetch(origin + '/about')).ok, 'preview startup', 60000);
  browser = await chrome(values.chrome, path.join(temporary, 'chromium'), browserLog.fd);
  const evaluate = browser.evaluate;
  const click = selector => evaluate(`const e=document.querySelector(${JSON.stringify(selector)});if(!e)throw Error('Missing control: '+${JSON.stringify(selector)});e.click()`);
  const tab = key => click(`[data-laytab="${key}"]`);
  const add = (key, value) => evaluate(`const f=document.querySelector('[data-add]');f.elements.key.value=${JSON.stringify(key)};f.elements.value.value=${JSON.stringify(value)};f.requestSubmit()`);
  const has = text => until(() => evaluate(`return document.body.innerText.includes(${JSON.stringify(text)})`), text);
  const settled = () => until(() => evaluate(`return ![...document.querySelectorAll('#page-warming')].length`), 'page warming');
  async function stage(journey, id, title, route, action) {
    const entry = { id, journey, title, route: route || '', status: 'passed', screenshot: `${id}.png` };
    const errorOffset = browser.errors.length;
    const dialogOffset = browser.dialogs.length;
    try {
      if (route) await browser.go(origin + route);
      if (route && ['/', '/scene'].includes(route)) await until(() => evaluate("return document.body.classList.contains('editing')"), 'editor ready');
      if (action) await action();
      await sleep(180);
      entry.observations = await evaluate(`return {text:document.body.innerText.replace(/just now|\\d+ seconds? ago/g,'recently').slice(0,18000),inputs:[...document.querySelectorAll('input:not([type=hidden]),textarea')].map(e=>({id:e.id,name:e.name,type:e.type,checked:e.checked,value:e.value})),overflow:document.documentElement.scrollWidth>innerWidth}`);
      entry.dialogs = browser.dialogs.slice(dialogOffset);
      if (browser.errors.length > errorOffset) throw new Error(browser.errors.slice(errorOffset).join('\n'));
    } catch (error) {
      entry.status = 'failed';
      entry.error = error.message;
      console.error(`${id}: ${error.message}`);
    }
    try {
      const comparison = await browser.screenshot(path.join(output, entry.screenshot));
      entry.comparison_sha256 = createHash('sha256').update(comparison).digest('hex');
      entry.sha256 = createHash('sha256').update(await fs.readFile(path.join(output, entry.screenshot))).digest('hex');
    } catch (error) { entry.status = 'failed'; entry.error = `${entry.error || ''} Screenshot: ${error.message}`; delete entry.screenshot; }
    manifest.stages.push(entry);
    await save();
    console.log(`${values.label}: ${id} ${entry.status}`);
  }
  await stage('preview', 'preview-desktop', 'Desktop preview', '/');
  for (const [where, title] of [['lan','Another device'],['web','Web explorer'],['phone','Phone QR code']]) await stage('preview', `preview-${where}`, title, `/?where=${where}`);
  await stage('scene', 'scene-info', 'Scene information', '/scene');
  await stage('scene', 'scene-editing', 'Edit title', '/scene', async () => {
    await evaluate("const e=document.querySelector('#edit-title');e.focus();const r=document.createRange();r.selectNodeContents(e);getSelection().removeAllRanges();getSelection().addRange(r)");
    await browser.send('Input.insertText', { text: 'Edited journey fixture' });
  });
  await stage('scene', 'scene-saved', 'Title saved', null, async () => { await evaluate("document.querySelector('#edit-title').blur()"); await has('Saved to scene.json'); });
  for (const [key,title] of [['parcels','Shape'],['spawns','Spawn points'],['perms','Permissions'],['server','Multiplayer']]) await stage('scene', `scene-${key}`, title, '/scene', () => tab(key));
  await stage('scene', 'spawn-editor', 'Spawn coordinates and camera', '/scene', async () => { await tab('spawns'); await click('[data-spawn="0"]'); });
  await stage('scene', 'media-toggle', 'Enable external media without hosts', '/scene', async () => { await tab('perms'); await click('[data-perm="ALLOW_MEDIA_HOSTNAMES"]'); });
  await stage('scene', 'media-reloaded', 'Reload media permission', '/scene', () => tab('perms'));
  await stage('storage', 'storage-empty', 'Empty scene storage', '/storage');
  await stage('storage', 'storage-added', 'Add JSON value', '/storage', async () => { await add('score', '{"score":1}'); await has('Saved score'); });
  await stage('storage', 'storage-second', 'Add second value', null, async () => { await add('name', '"original"'); await has('Saved name'); });
  await stage('storage', 'storage-drafts', 'Edit two values', null, async () => {
    await click('[data-key="score"] [data-act=edit]');
    await evaluate("document.querySelector('[data-key=score] textarea').value='{\"score\":99}'");
    await click('[data-key="name"] [data-act=edit]');
  });
  await stage('storage', 'storage-cancel', 'Cancel second value', null, async () => { await click('[data-key="name"] [data-act=cancel]'); await until(() => evaluate("return !document.querySelector('[data-key=name] textarea')"), 'row cancelled'); });
  await stage('storage', 'storage-player', 'Select player wallet', '/storage?tab=player');
  await stage('storage', 'storage-env', 'Environment values', '/storage?tab=env');
  await stage('storage', 'storage-env-added', 'Add public canary', '/storage?tab=env', async () => { await add('JOURNEY_CANARY', 'public-fixture-only'); await has('Saved JOURNEY_CANARY'); });
  await stage('storage', 'storage-env-revealed', 'Reveal public canary', null, () => click('[data-act=reveal]'));
  const drawer = () => evaluate("[...document.querySelectorAll('details')].find(e=>e.querySelector('summary').textContent.includes('Import and export')).open=true");
  await stage('storage-transfer', 'storage-transfer', 'Snapshot import and export', '/storage', drawer);
  await stage('storage-transfer', 'storage-import-selected', 'Choose empty snapshot', null, async () => {
    await evaluate(`const dt=new DataTransfer();dt.items.add(new File(['{"env":{},"world":{},"players":{}}'],'fixture.json',{type:'application/json'}));document.querySelector('[data-import] input[type=file]').files=dt.files`);
  });
  await stage('storage-transfer', 'storage-imported', 'Import snapshot', null, async () => { await evaluate("document.querySelector('[data-import]').requestSubmit()"); await until(() => evaluate("return document.querySelector('.toast')?.textContent.includes('snapshot')"), 'snapshot imported'); });
  await stage('storage-transfer', 'storage-invalid', 'Invalid snapshot recovery', null, async () => {
    await drawer();
    await evaluate("const dt=new DataTransfer();dt.items.add(new File(['invalid json'],'invalid.json',{type:'application/json'}));document.querySelector('[data-import] input[type=file]').files=dt.files;document.querySelector('[data-import]').requestSubmit()");
    await until(() => evaluate("return document.querySelector('.toast.toast--err') !== null"), 'invalid file error');
  });
  await stage('storage-transfer', 'storage-upstream', 'Choose upstream storage', '/storage', async () => { await click('input[name=upstream]'); await click('input[name=service][value=custom]'); });
  for (const [key,title] of [['world','World target'],['land','LAND target'],['history','Deployment history'],['help','Publishing help']]) await stage('target', `target-${key}`, title, '/target', async () => { await settled(); await click(`input[name=tgt][value=${key}]`); });
  await stage('deploy', 'deploy-signing', 'Prepared deployment review', '/deploy', () => until(() => evaluate("return !!document.querySelector('#sign-go')"), 'wallet handoff', 30000));
  await stage('deploy', 'deploy-no-wallet', 'Missing wallet recovery', null, async () => { await click('#sign-go'); await has('No wallet found'); });
  await stage('integrations', 'inspector', 'Inspector availability', '/inspector/');
  await stage('integrations', 'embedded-play', 'Embedded Play availability', '/play/');
  await stage('offline', 'preview-offline', 'Disconnected preview feedback', '/', () => evaluate("window.dispatchEvent(new Event('offline'))"));
  await browser.size(390, 844);
  for (const [key,route] of [['preview','/'],['scene','/scene'],['storage','/storage'],['target','/target'],['deploy','/deploy']]) await stage('phone', `phone-${key}`, `Phone: ${key}`, route, key === 'target' ? settled : undefined);
} catch (error) {
  manifest.errors.push(error.stack || error.message);
  console.error(error);
} finally {
  if (browser) { manifest.browserErrors = browser.errors; await browser.close(); }
  if (preview) { preview.kill('SIGTERM'); await Promise.race([once(preview,'exit'),sleep(3000)]); if(preview.exitCode===null)preview.kill('SIGKILL'); }
  if (server) await new Promise(resolve => server.close(resolve));
  await log.close(); await browserLog.close();
  await fs.writeFile(path.join(output,'fixture-requests.json'),JSON.stringify(requests,null,2));
  await save();
  await fs.rm(temporary, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
if (manifest.errors.length || manifest.browserErrors?.length || !manifest.stages.length || manifest.stages.some(stage => stage.status !== 'passed')) process.exitCode = 1;
