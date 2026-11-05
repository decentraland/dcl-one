// The wearable-preview iframe contract (@dcl/schemas preview messages) over the wasm module:
// options from the URL and `update` messages, `controller_request`s, and `ready`, `load`,
// `error`, `emote_event` and `controller_response` back to the host. Profiles resolve here, so
// the engine only ever receives urns.
import init, { start, preview_command } from './pkg/dcl_avatar_preview.js';

const RENDERER = 'bevy';
const BODY = {
  male: 'urn:decentraland:off-chain:base-avatars:BaseMale',
  female: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
};
const DEFAULT_RADIUS = 6; // metres at the default 26.2 degree field of view: a 2 m avatar framed
const DEFAULT_BACKGROUND = '#1a1720';

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
  camera: { alpha: 0, beta: 0.05, radius: DEFAULT_RADIUS, target: [0, 1, 0] },
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

async function fetchProfile(peer, profile) {
  const response = await fetch(`${peer}/lambdas/profiles/${profile}`);
  if (!response.ok) throw new Error(`profile ${profile}: ${response.status}`);
  const json = await response.json();
  const avatar = json?.avatars?.[0]?.avatar;
  if (!avatar) throw new Error(`profile ${profile}: no avatar`);
  return avatar;
}

async function resolveOutfit(o) {
  const peer = o.peerUrl || 'https://peer.decentraland.org';
  const outfit = { op: 'outfit', bodyShape: o.bodyShape || null, wearables: [], skin: o.skin, hair: o.hair, eyes: o.eyes };
  if (o.profile) {
    const avatar = await fetchProfile(peer, o.profile);
    outfit.bodyShape = o.bodyShape || avatar.bodyShape || null;
    outfit.wearables = [...(avatar.wearables || [])];
    outfit.skin = o.skin || (avatar.skin?.color ? hex(avatar.skin.color) : null);
    outfit.hair = o.hair || (avatar.hair?.color ? hex(avatar.hair.color) : null);
    outfit.eyes = o.eyes || (avatar.eyes?.color ? hex(avatar.eyes.color) : null);
  }
  outfit.wearables.push(...(o.urns || []));
  outfit.bodyShape = outfit.bodyShape || BODY.female;
  for (const k of ['skin', 'hair', 'eyes']) if (outfit[k] && !outfit[k].startsWith('#')) outfit[k] = '#' + outfit[k];
  return outfit;
}

// wearable-preview's zoom: 1..100 to a factor of 1..2.8 on the camera distance
function zoomFactor(zoom) {
  if (zoom === null || Number.isNaN(zoom)) return 1;
  const z = Math.max(1, Math.min(100, zoom));
  return 1 + ((z - 1) / 99) * 1.8;
}

function cameraFrom(o) {
  const c = state.camera;
  c.target = [o.offsetX || 0, 1 + (o.offsetY || 0), o.offsetZ || 0];
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
    command(await resolveOutfit(o));
    const emote = o.emote;
    if (emote && emote !== 'idle' && !o.disableDefaultEmotes) {
      state.emote.urn = emote;
      command({ op: 'emote', urn: emote, loop: false });
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

export async function boot({ canvas, baseDomain = null, log = () => {} } = {}) {
  state.log = log;
  send('ready', null);
  await init();
  start(canvas, baseDomain, onEngineEvent);
  state.engineReady = true;
  await apply();
}
