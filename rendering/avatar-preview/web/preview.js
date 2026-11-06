// The wearable-preview iframe contract (@dcl/schemas preview messages) over the wasm module:
// options from the URL and `update` messages, `controller_request`s, and `ready`, `load`,
// `error`, `emote_event` and `controller_response` back to the host. Profiles resolve here, so
// the engine only ever receives urns.
// The module is imported by build id (`pkg/build.json`, written by build.sh and always
// revalidated): a browser that cached an earlier module keeps it for other urls only.
let start;
let preview_command;
// The browser backend: one module per backend, as the engine cannot hold both. WebGPU where the
// browser offers an adapter, else WebGL2 (`pkg-gl/`); `backend=webgpu|webgl2` forces one.
async function backend() {
  const asked = new URL(location.href).searchParams.get('backend');
  if (asked === 'webgpu' || asked === 'webgl2') return asked;
  try {
    if (navigator.gpu && await navigator.gpu.requestAdapter()) return 'webgpu';
  } catch { /* no usable adapter */ }
  return 'webgl2';
}
async function loadModule() {
  const dir = (await backend()) === 'webgl2' ? './pkg-gl/' : './pkg/';
  let build = 'dev';
  try {
    const response = await fetch(new URL(`${dir}build.json`, import.meta.url), { cache: 'no-store' });
    if (response.ok) build = String((await response.json()).build);
  } catch { /* no build id: the module loads by its plain url */ }
  const glue = new URL(`${dir}dcl_avatar_preview.js?v=${build}`, import.meta.url);
  const wasm = new URL(`${dir}dcl_avatar_preview_bg.wasm?v=${build}`, import.meta.url);
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
const POSE_EMOTES = ['fashion', 'fashion-2', 'fashion-3', 'fashion-4', 'walk', 'run', 'love', 'jump'];
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
    panning: p.get('panning') !== 'false',
    wheelZoom: num(p.get('wheelZoom')),
    wheelStart: num(p.get('wheelStart')),
    wheelPrecision: num(p.get('wheelPrecision')),
    showThumbnailBoundaries: flag(p.get('showThumbnailBoundaries')),
    hideControls: flag(p.get('hideControls')),
    disableLoader: p.has('disableLoader') && p.get('disableLoader') !== 'false',
    disableSwitcher: flag(p.get('disableSwitcher')),
    mode: p.get('mode'),
  };
}

// --- state ---
const state = {
  options: parseOptions(location.search),
  overrides: {},
  engineReady: false,
  applying: null,
  emote: { urn: null, playing: false, paused: false, time: 0, length: 0, hasSound: false },
  // emote events wait for `load`: a host expects the avatar before its animation
  loadSent: false,
  heldEvents: [],
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
      ui.loading(false);
      send('load', { renderer: RENDERER });
      state.loadSent = true;
      for (const held of state.heldEvents.splice(0)) send('emote_event', held);
      break;
    case 'error':
      send('error', { message: event.message });
      break;
    case 'emote': {
      const e = state.emote;
      if (event.time != null) e.time = event.time;
      if (event.length != null) e.length = event.length;
      if (event.sound != null) e.hasSound = event.sound;
      const emit = (type, payload) => {
        const message = { type, payload };
        if (state.loadSent) send('emote_event', message); else state.heldEvents.push(message);
      };
      queueMicrotask(() => ui.emote());
      switch (event.event) {
        case 'play':
          e.playing = true; e.paused = false; e.time = 0;
          if (event.urn) e.urn = event.urn;
          emit('animation_play');
          // a pause sent with the start (a seek on a stopped emote) holds it from its first frame
          if (e.pendingPause) { e.pendingPause = false; e.paused = true; emit('animation_pause'); }
          break;
        case 'resume':
          e.playing = true; e.paused = false;
          emit('animation_play');
          break;
        case 'pause':
          if (!e.playing) { e.pendingPause = true; break; }
          e.paused = true;
          emit('animation_pause');
          break;
        case 'loop':
          emit('animation_loop');
          break;
        case 'playing':
          emit('animation_playing', { length: e.time });
          break;
        default: // end, interrupted
          e.playing = false; e.paused = false; e.pendingPause = false;
          emit('animation_end');
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

// which of `urns` are emotes: hosts pass an emote item by urn (the shop does), and only the
// catalyst knows what a urn names. Base avatars are wearables; anything the catalyst does not
// answer for stays a wearable, as before.
async function emoteUrns(peer, urns) {
  const ask = urns.filter((u) => !/^urn:decentraland:off-chain:base-avatars:/i.test(u));
  if (!ask.length) return new Set();
  try {
    const response = await fetch(`${peer}/content/entities/active`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: ask }),
    });
    if (!response.ok) return new Set();
    const emotes = new Set();
    for (const entity of await response.json()) {
      if (entity.type !== 'emote') continue;
      for (const pointer of entity.pointers || []) emotes.add(pointer.toLowerCase());
    }
    return new Set(ask.filter((u) => emotes.has(u.toLowerCase())));
  } catch { return new Set(); }
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

// as the Unity renderer plays its default emote: once in the marketplace, then the idle; looping
// in the builder
function registerPose(name, loop) {
  const urn = `${POSE_COLLECTION}:preview-${name}${loop ? '-loop' : ''}`;
  const file = `${name}.glb`;
  registerCollectible('emote', urn, {
    id: urn, name, description: '', thumbnail: '',
    emoteDataADR74: {
      representations: [{ bodyShapes: [BODY.male, BODY.female], mainFile: file, contents: [file] }],
      loop,
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
  // looked up first, added after the profile, which replaces the outfit's wearables
  const item = o.contractAddress && (o.itemId || o.tokenId) ? await fetchItem(o) : null;
  if (o.profile) {
    const avatar = await fetchProfile(peer, o.profile);
    outfit.bodyShape = o.bodyShape || avatar.bodyShape || null;
    outfit.wearables = [...(avatar.wearables || [])];
    outfit.skin = o.skin || (avatar.skin?.color ? hex(avatar.skin.color) : null);
    outfit.hair = o.hair || (avatar.hair?.color ? hex(avatar.hair.color) : null);
    outfit.eyes = o.eyes || (avatar.eyes?.color ? hex(avatar.eyes.color) : null);
  }
  // the items go first: the engine keeps the first wearable of each category, and an item takes
  // its category's slot from the profile's, as in the Unity renderer
  const items = [];
  if (item) {
    if (item.category === 'emote') itemEmote = item.urn; else items.push(item.urn);
  }
  const urns = o.urns || [];
  const emotes = await emoteUrns(peer, urns);
  for (const urn of urns) {
    if (emotes.has(urn)) itemEmote = itemEmote || urn; else items.push(urn);
  }
  for (const inline of await inlineDefinitions(o)) {
    if (inline.isEmote) itemEmote = itemEmote || inline.urn; else items.push(inline.urn);
  }
  outfit.wearables = [...items, ...outfit.wearables];
  outfit.bodyShape = outfit.bodyShape || BODY.female;
  for (const k of ['skin', 'hair', 'eyes']) if (outfit[k] && !outfit[k].startsWith('#')) outfit[k] = '#' + outfit[k];
  return { outfit, itemEmote };
}

// an emote option: an item's urn, a base emote name, or nothing playable (idle). A named default
// loops as the Unity renderer's does: once in the marketplace, then the idle; looping in the builder
async function emoteUrn(name, loop, peer) {
  if (!name) return null;
  if (name.startsWith('urn:')) return name;
  if (BASE_EMOTES[name]) return baseEmote(BASE_EMOTES[name], loop, peer);
  if (POSE_EMOTES.includes(name)) return registerPose(name, loop);
  if (name !== 'idle') console.warn(`emote "${name}" is unknown; idling`);
  return null;
}

// a catalyst base emote as a default: its own metadata says whether it loops (dance, money and clap
// do), so it is registered again under a preview urn with the mode's loop and the catalyst's files
async function baseEmote(id, loop, peer) {
  const urn = `${POSE_COLLECTION}:${id}`;
  try {
    const response = await fetch(`${peer}/content/entities/active`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: [urn] }),
    });
    const [entity] = response.ok ? await response.json() : [];
    const data = entity?.metadata?.emoteDataADR74;
    if (!data) return id;
    if (!!data.loop === loop) return urn;
    const files = Object.fromEntries((entity.content || []).map((c) => [c.file, `${peer}/content/contents/${c.hash}`]));
    const copy = `${POSE_COLLECTION}:preview-${id}${loop ? '-loop' : '-once'}`;
    registerCollectible('emote', copy, { ...entity.metadata, id: copy, emoteDataADR74: { ...data, loop } }, files);
    return copy;
  } catch { return id; }
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
  const mode = modeOf(o);
  // the Unity renderer: input in marketplace and builder only; auto-rotation in marketplace, and
  // there only when no emote item plays; a builder avatar turns four times faster under the drag
  const interactive = o.camera !== 'static' && (mode === 'marketplace' || mode === 'builder');
  return {
    op: 'camera',
    alpha: c.alpha, beta: c.beta, radius: c.radius, target: c.target,
    orthographic: o.projection === 'orthographic',
    autoRotate: interactive && mode === 'marketplace' && !state.itemEmote && !o.disableAutoRotate,
    autoRotateSpeed: o.autoRotateSpeed ?? 0.2,
    interactive,
    dragSpeed: ((mode === 'builder' ? 2 : 0.5) * Math.PI) / 180,
    lockAlpha: !!o.lockAlpha,
    lockRadius: !!o.lockRadius,
    panning: o.panning !== false,
    wheelPrecision: o.wheelPrecision ?? 100,
    reset: true,
  };
}

// the Unity renderer's modes; an unknown or missing one is marketplace, as the wrapper sends
const MODES = ['marketplace', 'builder', 'profile', 'authentication', 'jesus', 'configurator'];
const modeOf = (o) => (MODES.includes(o.mode) ? o.mode : 'marketplace');

// the wrapper hid the Unity renderer's controls whenever the host frames the canvas itself
const hidesControls = (o) => !!o.hideControls || o.zoom != null || o.wheelZoom != null
  || !!o.showThumbnailBoundaries || o.camera === 'static';

function sendCamera() {
  const c = state.camera;
  command({ op: 'camera', alpha: c.alpha, beta: c.beta, radius: c.radius, target: c.target });
}

async function apply() {
  if (!state.engineReady) return;
  const o = options();
  const run = (async () => {
    state.loadSent = false;
    state.heldEvents = [];
    ui.loading(true, o);
    const { outfit, itemEmote } = await resolveOutfit(o);
    state.itemEmote = itemEmote;
    ui.configure(o);
    command(cameraFrom(o));
    command({ op: 'background', color: o.disableBackground ? null : (o.background ? (o.background.startsWith('#') ? o.background : '#' + o.background) : DEFAULT_BACKGROUND) });
    command(outfit);
    const peer = o.peerUrl || (o.env === 'dev' ? PEER.dev : PEER.prod);
    const urn = itemEmote
      || (o.disableDefaultEmotes ? null : await emoteUrn(o.emote, modeOf(o) === 'builder', peer));
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
    getLength: async () => state.emote.length,
    isPlaying: async () => state.emote.playing && !state.emote.paused,
    // like the Unity renderer: seeking a stopped emote starts it and holds it at that time
    goTo: async (seconds) => {
      const e = state.emote;
      if (!e.urn) throw new Error('no emote to seek');
      if (!e.playing) {
        command({ op: 'emote', urn: e.urn, loop: false });
        command({ op: 'pauseEmote' });
      }
      command({ op: 'seekEmote', seconds: Math.max(0, Number(seconds) || 0) });
    },
    play: async () => {
      const e = state.emote;
      if (!e.urn) throw new Error('no emote to play');
      if (e.playing && e.paused) command({ op: 'resumeEmote' });
      else if (!e.playing) command({ op: 'emote', urn: e.urn, loop: false });
    },
    pause: async () => { if (state.emote.playing) command({ op: 'pauseEmote' }); },
    stop: async () => { command({ op: 'stopEmote' }); },
    enableSound: async () => { state.bridgeSound = true; ui.sound(); },
    disableSound: async () => { state.bridgeSound = false; ui.sound(); },
    hasSound: async () => state.emote.hasSound,
    isSocialEmote: async () => false,
    getSocialEmoteAnimations: async () => null,
    getPlayingSocialEmoteAnimation: async () => null,
  },
  physics: {
    setSpringBonesParams: () => unsupported('physics.setSpringBonesParams'),
  },
};

// --- the Unity renderer's controls (index.html lays them out) ---
state.bridgeSound = true;
state.muted = false;
const ui = (() => {
  const body = document.body;
  const $ = (id) => document.getElementById(id);
  const scale = () => {
    // UI Toolkit's ScaleWithScreenSize against 700x700, match 0.5: the geometric mean
    const s = Math.sqrt(window.innerWidth * window.innerHeight) / 700;
    document.documentElement.style.setProperty('--s', String(s));
  };
  window.addEventListener('resize', scale);
  scale();
  // right drag pans: the pan cursor while it is held, and no context menu
  window.addEventListener('contextmenu', (e) => e.preventDefault());
  window.addEventListener('pointerdown', (e) => { if (e.button === 2) body.classList.add('panning'); });
  window.addEventListener('pointerup', (e) => { if (e.button === 2) body.classList.remove('panning'); });

  let lastClick = 0;
  const playStop = $('play-stop');
  playStop?.addEventListener('click', () => {
    const now = performance.now();
    if (now - lastClick < 500) return; // the renderer's debounce
    lastClick = now;
    const e = state.emote;
    if (e.playing && !e.paused) {
      // the in-canvas stop returns to idle; the controller's stop holds the first frame
      command({ op: 'stopEmote' });
    } else if (e.playing && e.paused) {
      command({ op: 'resumeEmote' });
    } else if (e.urn) {
      command({ op: 'emote', urn: e.urn, loop: false });
    }
  });
  const mute = $('mute');
  mute?.addEventListener('click', () => { state.muted = !state.muted; api.sound(); });

  const api = {
    loading(on, o) {
      body.classList.toggle('loading', on);
      if (o) body.classList.toggle('no-loader', !!o.disableLoader);
    },
    configure(o) {
      const marketplace = modeOf(o) === 'marketplace';
      body.classList.toggle('hide-controls', hidesControls(o));
      body.classList.toggle('emote-item', marketplace && !!state.itemEmote);
      body.classList.toggle('thumbnail-bounds', !!o.showThumbnailBoundaries);
      api.emote();
    },
    emote() {
      const e = state.emote;
      const running = e.playing && !e.paused;
      playStop?.classList.toggle('stopped', !running);
      const label = playStop?.querySelector('.label');
      if (label) label.textContent = running ? 'STOP EMOTE' : 'PLAY EMOTE';
      body.classList.toggle('emote-sound', !!e.hasSound);
    },
    sound() {
      mute?.classList.toggle('muted', state.muted);
      command({ op: 'sound', enabled: state.bridgeSound && !state.muted });
    },
  };
  return api;
})();

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
