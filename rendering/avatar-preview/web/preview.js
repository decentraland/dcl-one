// The wearable-preview iframe contract (@dcl/schemas preview messages) over the wasm module:
// options from the URL and `update` messages, `controller_request`s, and `ready`, `load`,
// `error`, `emote_event` and `controller_response` back to the host. Profiles resolve here, so
// the engine only ever receives urns.
// The module is imported by build id (`pkg/build.json`, written by build.sh and always
// revalidated): a browser that cached an earlier module keeps it for other urls only.
let start;
let preview_command;
async function loadModule() {
  let build = 'dev';
  try {
    const response = await fetch(new URL('./pkg/build.json', import.meta.url), { cache: 'no-store' });
    if (response.ok) build = String((await response.json()).build);
  } catch { /* no build id: the module loads by its plain url */ }
  const glue = new URL(`./pkg/dcl_avatar_preview.js?v=${build}`, import.meta.url);
  const wasm = new URL(`./pkg/dcl_avatar_preview_bg.wasm?v=${build}`, import.meta.url);
  const module = await import(glue.href);
  await module.default(wasm);
  ({ start, preview_command } = module);
}

const RENDERER = 'bevy';
const BODY = {
  male: 'urn:decentraland:off-chain:base-avatars:BaseMale',
  female: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
};
// the default framing, measured against wearable-preview's Unity renderer: the avatar fills most
// of the panel's height, seen from slightly above
const DEFAULT_RADIUS = 4.6; // metres at the default 26.2 degree field of view
const DEFAULT_TARGET_Y = 0.95;
const DEFAULT_BETA = 0.12;
const DEFAULT_BACKGROUND = '#1a1720';
// wearable-preview's emote names (PreviewEmote) that the catalyst's base-emotes collection has;
// the rest (idle, walk, run, jump, fashion poses, love) are its own files and fall back to idle
const BASE_EMOTES = {
  clap: 'clap', dab: 'dab', dance: 'dance', money: 'money', wave: 'wave',
  'fist-pump': 'fistpump', 'head-explode': 'headexplode',
};
// wearable-preview's poses that are its own files, not catalyst emotes: shipped in web/emotes/ and
// registered with the engine under this collection the first time they are asked for
const POSE_EMOTES = {
  fashion: true, 'fashion-2': true, 'fashion-3': true, 'fashion-4': true,
  walk: true, run: true, love: false, jump: false, // name: loops
};
// the engine accepts off-chain urns in its known collections only; an inline registration replaces
// the catalyst lookup for that exact urn, so these names never reach the content server
const POSE_COLLECTION = 'urn:decentraland:off-chain:base-emotes';
const inlineUrn = (id, isEmote) => (/^urn:decentraland:/i.test(id) ? id
  : `urn:decentraland:off-chain:${isEmote ? 'base-emotes' : 'base-avatars'}:preview-${String(id).replace(/[^a-zA-Z0-9_-]/g, '-')}`);
const MARKETPLACE = { prod: 'https://marketplace-api.decentraland.org', dev: 'https://marketplace-api.decentraland.zone' };
const PEER = { prod: 'https://peer.decentraland.org', dev: 'https://peer.decentraland.zone' };

const parent = window.parent || window;
const send = (type, payload) => parent.postMessage({ type, payload }, '*');

// --- options (the wearable-preview `useOptions` subset the renderer understands) ---
const num = (v) => (v === null || v === undefined || v === '' ? null : Number.parseFloat(v));
const flag = (v) => v !== null && v !== 'false';
export function parseOptions(search) {
  const p = new URLSearchParams(search);
  const bodyShape = p.get('bodyShape');
  return {
    contractAddress: p.get('contract'),
    tokenId: p.get('token'),
    itemId: p.get('item'),
    profile: p.get('profile'),
    bodyShape: bodyShape === 'female' || bodyShape === BODY.female ? BODY.female
      : bodyShape === 'male' || bodyShape === BODY.male ? BODY.male : null,
    skin: p.get('skin'),
    hair: p.get('hair'),
    eyes: p.get('eyes'),
    urns: p.getAll('urn'),
    urls: p.getAll('url'),
    base64s: p.getAll('base64'),
    emote: p.get('emote'),
    camera: p.get('camera'),
    projection: p.get('projection'),
    zoom: num(p.get('zoom')),
    zoomScale: num(p.get('zoomScale')),
    autoRotateSpeed: num(p.get('autoRotateSpeed')),
    offsetX: num(p.get('offsetX')), offsetY: num(p.get('offsetY')), offsetZ: num(p.get('offsetZ')),
    cameraX: num(p.get('cameraX')), cameraY: num(p.get('cameraY')), cameraZ: num(p.get('cameraZ')),
    background: p.get('background'),
    disableBackground: flag(p.get('disableBackground')) || p.has('transparentBackground'),
    disableAutoRotate: flag(p.get('disableAutoRotate')) || p.get('centerBoundingBox') === 'false',
    disableAutoCenter: flag(p.get('disableAutoCenter')) || p.get('centerBoundingBox') === 'false',
    disableDefaultEmotes: flag(p.get('disableDefaultEmotes')),
    disableDefaultWearables: flag(p.get('disableDefaultWearables')),
    peerUrl: p.get('peerUrl'),
    marketplaceServerUrl: p.get('marketplaceServerUrl') || p.get('nftServerUrl'),
    env: p.get('env'),
    type: p.get('type'),
    lockAlpha: p.get('lockAlpha') === 'true',
    lockBeta: p.get('lockBeta') === 'true',
    lockRadius: p.get('lockRadius') === 'true',
  };
}

// --- state ---
const state = {
  options: parseOptions(location.search),
  overrides: {},
  engineReady: false,
  applying: null,
  emote: { urn: null, playing: false },
  pending: new Map(), // request id -> { resolve, reject }
  camera: { alpha: 0, beta: DEFAULT_BETA, radius: DEFAULT_RADIUS, target: [0, DEFAULT_TARGET_Y, 0] },
  log: () => {},
};
const options = () => ({ ...state.options, ...state.overrides });
const nextId = (() => { let n = 0; return () => `p${++n}`; })();

function command(cmd) {
  state.log(`-> ${JSON.stringify(cmd)}`);
  preview_command(JSON.stringify(cmd));
}

function request(op, extra = {}) {
  const id = nextId();
  return new Promise((resolve, reject) => {
    state.pending.set(id, { resolve, reject });
    command({ op, id, ...extra });
    setTimeout(() => {
      if (state.pending.delete(id)) reject(new Error(`${op} timed out`));
    }, 15000);
  });
}

// --- engine events ---
function onEngineEvent(json) {
  const event = JSON.parse(json);
  state.log(`<- ${json.length > 200 ? json.slice(0, 200) + '…' : json}`);
  switch (event.type) {
    case 'load':
      send('load', { renderer: RENDERER });
      break;
    case 'error':
      send('error', { message: event.message });
      break;
    case 'emote': {
      if (event.event === 'play') {
        state.emote.playing = true;
        if (event.urn) state.emote.urn = event.urn;
        send('emote_event', { type: 'animation_play', payload: undefined });
      } else {
        state.emote.playing = false;
        send('emote_event', { type: 'animation_end', payload: undefined });
      }
      break;
    }
    case 'screenshot':
    case 'metrics': {
      const pending = state.pending.get(event.id);
      if (!pending) break;
      state.pending.delete(event.id);
      pending.resolve(event);
      break;
    }
  }
}

// --- resolving options into commands ---
const hex = (c) => '#' + [c.r, c.g, c.b].map((v) => Math.round(Math.max(0, Math.min(1, v)) * 255).toString(16).padStart(2, '0')).join('');

const getJson = async (url) => {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  return response.json();
};

// `default` picks one of the stock profiles, as wearable-preview does; a content hash is a profile
// entity; anything else is an address the lambdas resolve
async function fetchProfile(peer, profile) {
  if (profile === 'default') profile = `default${Math.floor(Math.random() * 159) + 1}`;
  const json = /^(bafy|Qm)/.test(profile)
    ? (await getJson(`${peer}/content/contents/${profile}`)).metadata
    : await getJson(`${peer}/lambdas/profiles/${profile}`);
  const avatar = json?.avatars?.[0]?.avatar;
  if (!avatar) throw new Error(`profile ${profile}: no avatar`);
  return avatar;
}

// a marketplace item (contract + item id) or NFT (contract + token id): its urn and category
async function fetchItem(o) {
  const server = o.marketplaceServerUrl || (o.env === 'dev' ? MARKETPLACE.dev : MARKETPLACE.prod);
  if (o.itemId) {
    const { data } = await getJson(`${server}/v1/items?contractAddress=${o.contractAddress}&itemId=${o.itemId}`);
    if (!data?.length) throw new Error(`item ${o.contractAddress}:${o.itemId} not found`);
    return { urn: data[0].urn, category: data[0].category };
  }
  const { data } = await getJson(`${server}/v1/nfts?contractAddress=${o.contractAddress}&tokenId=${o.tokenId}`);
  if (!data?.length) throw new Error(`nft ${o.contractAddress}:${o.tokenId} not found`);
  const nft = data[0].nft;
  return { urn: nft.urn, category: nft.category };
}

// --- items that are not on the catalyst: registered with the engine by definition and file urls ---
const registered = new Map(); // urn -> registration key, so an unchanged item is sent once
const dataUrl = (json) => `data:application/json,${encodeURIComponent(JSON.stringify(json))}`;

function registerCollectible(kind, urn, metadata, files) {
  const key = JSON.stringify([kind, metadata, files]);
  if (registered.get(urn) === key) return;
  registered.set(urn, key);
  const definition = dataUrl({ id: urn, pointers: [urn], content: [], metadata });
  command({ op: 'collectible', kind, urn, definition, files });
}

function registerPose(name) {
  const urn = `${POSE_COLLECTION}:preview-${name}`;
  const file = `${name}.glb`;
  registerCollectible('emote', urn, {
    id: urn, name, description: '', thumbnail: '',
    emoteDataADR74: {
      representations: [{ bodyShapes: [BODY.male, BODY.female], mainFile: file, contents: [file] }],
      loop: POSE_EMOTES[name],
    },
  }, { [file]: new URL(`./emotes/${file}`, import.meta.url).href });
  return urn;
}

// a WearableDefinition or EmoteDefinition (@dcl/schemas): representations carry their files as
// `{key, url}` (or `{key, blob}` for unpublished items); the engine takes names and a url map
function registerDefinition(def) {
  const isEmote = !!def.emoteDataADR74;
  const files = {};
  const toNames = (representations) => (representations || []).map((r) => {
    const contents = (r.contents || []).map((c) => {
      if (typeof c === 'string') return c;
      files[c.key] = c.url || (c.blob ? URL.createObjectURL(c.blob) : '');
      return c.key;
    });
    return isEmote ? { ...r, contents } : { overrideHides: [], overrideReplaces: [], ...r, contents };
  });
  const metadata = isEmote
    ? { ...def, name: def.name || '', description: def.description || '', thumbnail: def.thumbnail || '',
        emoteDataADR74: { ...def.emoteDataADR74, representations: toNames(def.emoteDataADR74.representations) } }
    : { ...def, thumbnail: def.thumbnail || '',
        data: { tags: [], hides: [], replaces: [], ...def.data, representations: toNames(def.data?.representations) } };
  const urn = inlineUrn(def.id, isEmote);
  registerCollectible(isEmote ? 'emote' : 'wearable', urn, { ...metadata, id: urn }, files);
  return { urn, isEmote };
}

async function inlineDefinitions(o) {
  const defs = [];
  for (const url of o.urls || []) defs.push(await getJson(url));
  for (const b64 of o.base64s || []) defs.push(JSON.parse(atob(b64)));
  if (o.blob) defs.push(o.blob);
  return defs.map(registerDefinition);
}

async function resolveOutfit(o) {
  const peer = o.peerUrl || (o.env === 'dev' ? PEER.dev : PEER.prod);
  const outfit = { op: 'outfit', bodyShape: o.bodyShape || null, wearables: [], skin: o.skin, hair: o.hair, eyes: o.eyes };
  let itemEmote = null;
  if (o.contractAddress && (o.itemId || o.tokenId)) {
    const item = await fetchItem(o);
    if (item.category === 'emote') itemEmote = item.urn; else outfit.wearables.push(item.urn);
  }
  if (o.profile) {
    const avatar = await fetchProfile(peer, o.profile);
    outfit.bodyShape = o.bodyShape || avatar.bodyShape || null;
    outfit.wearables = [...(avatar.wearables || [])];
    outfit.skin = o.skin || (avatar.skin?.color ? hex(avatar.skin.color) : null);
    outfit.hair = o.hair || (avatar.hair?.color ? hex(avatar.hair.color) : null);
    outfit.eyes = o.eyes || (avatar.eyes?.color ? hex(avatar.eyes.color) : null);
  }
  outfit.wearables.push(...(o.urns || []));
  for (const item of await inlineDefinitions(o)) {
    if (item.isEmote) itemEmote = itemEmote || item.urn; else outfit.wearables.push(item.urn);
  }
  outfit.bodyShape = outfit.bodyShape || BODY.female;
  for (const k of ['skin', 'hair', 'eyes']) if (outfit[k] && !outfit[k].startsWith('#')) outfit[k] = '#' + outfit[k];
  return { outfit, itemEmote };
}

// an emote option: an item's urn, a base emote name, or nothing playable (idle)
function emoteUrn(name) {
  if (!name) return null;
  if (name.startsWith('urn:')) return name;
  if (BASE_EMOTES[name]) return BASE_EMOTES[name];
  if (name in POSE_EMOTES) return registerPose(name);
  if (name !== 'idle') console.warn(`emote "${name}" is unknown; idling`);
  return null;
}

// wearable-preview's zoom: 1..100 to a factor of 1..2.8 on the camera distance
function zoomFactor(zoom) {
  if (zoom === null || Number.isNaN(zoom)) return 1;
  const z = Math.max(1, Math.min(100, zoom));
  return 1 + ((z - 1) / 99) * 1.8;
}

function cameraFrom(o) {
  const c = state.camera;
  c.target = [o.offsetX || 0, DEFAULT_TARGET_Y + (o.offsetY || 0), o.offsetZ || 0];
  if (o.cameraX !== null || o.cameraY !== null || o.cameraZ !== null) {
    const dx = (o.cameraX || 0) - c.target[0], dy = (o.cameraY || 0) - c.target[1], dz = (o.cameraZ || 0) - c.target[2];
    c.radius = Math.max(0.5, Math.hypot(dx, dy, dz));
    c.alpha = Math.atan2(dx, dz);
    c.beta = Math.asin(dy / c.radius);
  } else {
    c.radius = (DEFAULT_RADIUS / zoomFactor(o.zoom)) * (o.zoomScale || 1);
  }
  return {
    op: 'camera',
    alpha: c.alpha, beta: c.beta, radius: c.radius, target: c.target,
    orthographic: o.projection === 'orthographic',
    autoRotate: !o.disableAutoRotate,
    autoRotateSpeed: o.autoRotateSpeed ?? 0.25,
    interactive: o.camera !== 'static',
  };
}

function sendCamera() {
  const c = state.camera;
  command({ op: 'camera', alpha: c.alpha, beta: c.beta, radius: c.radius, target: c.target });
}

async function apply() {
  if (!state.engineReady) return;
  const o = options();
  const run = (async () => {
    command(cameraFrom(o));
    command({ op: 'background', color: o.disableBackground ? null : (o.background ? (o.background.startsWith('#') ? o.background : '#' + o.background) : DEFAULT_BACKGROUND) });
    const { outfit, itemEmote } = await resolveOutfit(o);
    command(outfit);
    const urn = itemEmote || (o.disableDefaultEmotes ? null : emoteUrn(o.emote));
    if (urn) {
      state.emote.urn = urn;
      command({ op: 'emote', urn, loop: false });
    }
  })();
  state.applying = run;
  try { await run; } catch (e) { console.error(e); send('error', { message: String(e?.message ?? e) }); }
}

// --- controller ---
const unsupported = (what) => Promise.reject(new Error(`${what} is not supported by the ${RENDERER} renderer yet`));
const controller = {
  scene: {
    getScreenshot: async (width, height) => `data:image/png;base64,${(await request('screenshot', { width: Math.max(1, width | 0), height: Math.max(1, height | 0) })).png}`,
    getMetrics: async () => { const m = await request('metrics'); return { triangles: m.triangles, materials: m.materials, textures: m.textures, meshes: m.meshes, bodies: m.bodies, entities: m.entities }; },
    changeZoom: async (delta) => { state.camera.radius = Math.max(0.5, state.camera.radius - (delta || 0)); sendCamera(); },
    panCamera: async ({ x = 0, y = 0, z = 0 } = {}) => { const t = state.camera.target; state.camera.target = [t[0] + x, t[1] + y, t[2] + z]; sendCamera(); },
    changeCameraPosition: async ({ alpha = 0, beta = 0, radius = 0 } = {}) => { const c = state.camera; c.alpha += alpha; c.beta += beta; c.radius = Math.max(0.5, c.radius + radius); sendCamera(); },
    setUsername: async () => {},
    cleanup: async () => { command({ op: 'stopEmote' }); },
  },
  emote: {
    getLength: () => unsupported('emote.getLength'),
    isPlaying: async () => state.emote.playing,
    goTo: () => unsupported('emote.goTo'),
    play: async () => { if (!state.emote.urn) throw new Error('no emote to play'); command({ op: 'emote', urn: state.emote.urn, loop: false }); },
    pause: () => unsupported('emote.pause'),
    stop: async () => { command({ op: 'stopEmote' }); },
    enableSound: async () => {},
    disableSound: async () => {},
    hasSound: async () => false,
    isSocialEmote: async () => false,
    getSocialEmoteAnimations: async () => null,
    getPlayingSocialEmoteAnimation: async () => null,
  },
  physics: {
    setSpringBonesParams: () => unsupported('physics.setSpringBonesParams'),
  },
};

window.addEventListener('message', async (event) => {
  const data = event.data;
  if (!data || typeof data !== 'object') return;
  if (data.type === 'update' && data.payload && typeof data.payload.options === 'object') {
    state.overrides = { ...state.overrides, ...data.payload.options };
    apply();
  } else if (data.type === 'bevy_light' && data.payload && typeof data.payload === 'object') {
    // not part of the preview protocol: web/compare.html tunes the light rig with it
    command({ op: 'light', ...data.payload });
  } else if (data.type === 'controller_request' && data.payload && typeof data.payload.id === 'string') {
    const { id, namespace, method, params } = data.payload;
    const ns = controller[namespace];
    if (!ns) return send('controller_response', { id, ok: false, error: 'Invalid namespace' });
    if (typeof ns[method] !== 'function') return send('controller_response', { id, ok: false, error: 'Invalid method' });
    try {
      send('controller_response', { id, ok: true, result: await ns[method](...(Array.isArray(params) ? params : [])) });
    } catch (e) {
      send('controller_response', { id, ok: false, error: String(e?.message ?? e) });
    }
  }
});

// the engine's catalyst follows the host's `peerUrl` (`peer.<domain>`), unless the page says
function baseDomainFor(o) {
  const peer = o.peerUrl || (o.env === 'dev' ? PEER.dev : PEER.prod);
  try {
    const host = new URL(peer).hostname;
    return host.startsWith('peer.') ? host.slice('peer.'.length) : null;
  } catch { return null; }
}

// a page-level failure (a panic unwinding out of the module, a script error) reaches the host as
// `error`, once, so an embedding page can show its error state instead of waiting for `load`
let reportedFatal = false;
function reportFatal(message) {
  if (reportedFatal) return;
  reportedFatal = true;
  send('error', { message });
}
window.addEventListener('error', (e) => reportFatal(`${e.message} (${e.filename}:${e.lineno})`));
window.addEventListener('unhandledrejection', (e) => reportFatal(String(e.reason?.message ?? e.reason)));

export async function boot({ canvas, baseDomain = null, log = () => {} } = {}) {
  state.log = log;
  send('ready', null);
  try {
    await loadModule();
    start(canvas, baseDomain || baseDomainFor(options()), onEngineEvent, new URLSearchParams(location.search).get('flags'));
  } catch (e) {
    reportFatal(`renderer failed to start: ${e?.message ?? e}`);
    throw e;
  }
  state.engineReady = true;
  await apply();
}
