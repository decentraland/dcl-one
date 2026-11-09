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
const DEFAULT_FOV = 26.2;
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
    disableFace: flag(p.get('disableFace')),
    showSceneBoundaries: flag(p.get('showSceneBoundaries')) || flag(p.get('showAnimationReference')),
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

// before the engine has started there is nothing to command: the page sends its whole state once
// it has (`apply`), so a controller call in between is dropped
function command(cmd) {
  if (typeof preview_command !== 'function') return;
  state.log(`-> ${JSON.stringify(cmd)}`);
  preview_command(JSON.stringify(cmd));
}

function request(op, extra = {}) {
  // the Unity renderer's answer before the preview has loaded
  if (!state.engineReady) return Promise.reject(new Error('The preview is reloading'));
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
    case 'metrics':
    case 'requestFailed': {
      const pending = state.pending.get(event.id);
      if (!pending) break;
      state.pending.delete(event.id);
      if (event.type === 'requestFailed') pending.reject(new Error(event.message));
      else pending.resolve(event);
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

// what the catalyst says `urns` are: emote or wearable, its category and the body shapes it has
// a representation for. Hosts pass emote items by urn (the shop does), and only the catalyst knows
// what a urn names; a urn it does not answer for stays a wearable, as before.
async function describe(peer, urns) {
  const found = new Map(); // lowercased pointer -> { isEmote, category, bodyShapes }
  if (!urns.length) return found;
  try {
    const response = await fetch(`${peer}/content/entities/active`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers: urns }),
    });
    if (!response.ok) return found;
    for (const entity of await response.json()) {
      const isEmote = entity.type === 'emote';
      const data = isEmote ? entity.metadata?.emoteDataADR74 : entity.metadata?.data;
      const info = {
        isEmote,
        category: data?.category ?? null,
        bodyShapes: (data?.representations || []).flatMap((r) => r.bodyShapes || []),
      };
      for (const pointer of entity.pointers || []) found.set(pointer.toLowerCase(), info);
    }
  } catch { /* undescribed: wearables */ }
  return found;
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
  const data = isEmote ? def.emoteDataADR74 : def.data;
  return {
    urn, isEmote, category: data?.category ?? null,
    bodyShapes: (data?.representations || []).flatMap((r) => r.bodyShapes || []),
  };
}

async function inlineDefinitions(o) {
  const defs = [];
  for (const url of o.urls || []) defs.push(await getJson(url));
  for (const b64 of o.base64s || []) defs.push(JSON.parse(atob(b64)));
  if (o.blob) defs.push(o.blob);
  return defs.map(registerDefinition);
}

// the Unity renderer's builder dresses a base avatar, not a profile: the categories nothing
// requested covers get these, by body shape
const BUILDER_DEFAULTS = {
  hair: ['casual_hair_01', 'standard_hair'],
  upper_body: ['green_hoodie', 'f_sweater'],
  lower_body: ['brown_pants', 'f_jeans'],
  feet: ['sneakers', 'bun_shoes'],
};
const BUILDER_FACE = {
  eyebrows: ['eyebrows_00', 'f_eyebrows_00'],
  mouth: ['mouth_00', 'f_mouth_00'],
  eyes: ['eyes_00', 'f_eyes_00'],
};
const BUILDER_SKIN = '#cc9b76';

// builder mode, as the Unity renderer loads it: the page's own items, then the requested urns (the
// page's win a category), dressed with base wearables where nothing covers it (and the face's
// unless `disableFace`), a male body unless one is asked for, the builder's skin and black hair
// and eyes. Only an emote of the page's own plays; a urn emote is not worn.
async function resolveBuilder(o, peer) {
  const bodyShape = o.bodyShape || BODY.male;
  const male = bodyShape === BODY.male;
  const urns = o.urns || [];
  const described = await describe(peer, urns);
  const requested = urns.map((urn) => ({ urn, isEmote: false, category: null, bodyShapes: [], ...described.get(urn.toLowerCase()) }));
  const local = await inlineDefinitions(o);
  const wearables = [...local, ...requested].filter((i) => !i.isEmote);
  const covered = new Set(wearables.map((i) => i.category).filter(Boolean));
  const defaults = [];
  if (!o.disableDefaultWearables) {
    const table = o.disableFace ? BUILDER_DEFAULTS : { ...BUILDER_DEFAULTS, ...BUILDER_FACE };
    for (const [category, [m, f]] of Object.entries(table)) {
      if (!covered.has(category)) defaults.push(`urn:decentraland:off-chain:base-avatars:${male ? m : f}`);
    }
  }
  const hexOf = (v, fallback) => (v ? (v.startsWith('#') ? v : '#' + v) : fallback);
  return {
    outfit: {
      op: 'outfit', bodyShape,
      wearables: [...wearables.map((i) => i.urn), ...defaults],
      skin: hexOf(o.skin, BUILDER_SKIN), hair: hexOf(o.hair, '#000000'), eyes: hexOf(o.eyes, '#000000'),
      hideFace: !!o.disableFace,
    },
    itemEmote: local.find((i) => i.isEmote)?.urn ?? null,
    solo: null,
    locked: false,
  };
}

// the profile modes, as the Unity renderer loads them: the profile's avatar as it is, whatever
// urns or item the page also names (`default1` when it names no profile)
async function resolveProfile(o, peer) {
  const avatar = await fetchProfile(peer, o.profile || 'default1');
  const color = (c) => (c?.color ? hex(c.color) : null);
  return {
    outfit: {
      op: 'outfit',
      bodyShape: avatar.bodyShape || BODY.female,
      wearables: [...(avatar.wearables || [])],
      skin: color(avatar.skin), hair: color(avatar.hair), eyes: color(avatar.eyes),
      hideFace: !!o.disableFace,
    },
    itemEmote: null,
    solo: null,
    locked: false,
  };
}

async function resolveOutfit(o) {
  const peer = o.peerUrl || (o.env === 'dev' ? PEER.dev : PEER.prod);
  if (modeOf(o) === 'builder') return resolveBuilder(o, peer);
  if (PROFILE_MODES.includes(modeOf(o))) return resolveProfile(o, peer);
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
  const urns = [...(item ? [item.urn] : []), ...(o.urns || [])];
  const described = await describe(peer, urns);
  const items = urns.map((urn) => ({ urn, isEmote: false, category: null, bodyShapes: [],
    ...described.get(urn.toLowerCase()), ...(urn === item?.urn && item.category === 'emote' ? { isEmote: true } : {}) }));
  items.push(...await inlineDefinitions(o));
  itemEmote = items.find((i) => i.isEmote)?.urn ?? null;
  const wearables = items.filter((i) => !i.isEmote);
  outfit.bodyShape = outfit.bodyShape || BODY.female;
  const fits = (i) => !i.bodyShapes.length || i.bodyShapes.some((b) => b.toLowerCase() === outfit.bodyShape.toLowerCase());
  // the item view exists for one wearable on its own (an emote or several items only show worn),
  // locked to it when the item has no representation for the avatar's body shape
  const solo = modeOf(o) === 'marketplace' && !itemEmote && wearables.length === 1 ? wearables[0] : null;
  const locked = !!solo && !fits(solo);
  const worn = wearables.filter(fits);
  outfit.wearables = [...worn.map((i) => i.urn), ...outfit.wearables];
  // shown even where another wearable would hide it: the item is what the page is about
  outfit.forceRender = [...new Set(worn.map((i) => i.category).filter(Boolean))];
  outfit.hideFace = !!o.disableFace;
  for (const k of ['skin', 'hair', 'eyes']) if (outfit[k] && !outfit[k].startsWith('#')) outfit[k] = '#' + outfit[k];
  return { outfit, itemEmote, solo, locked };
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

// The Unity renderer's zoom range (its `ZoomRange.FromOptions`), as factors over the framed view
// (closer is larger). With neither `zoom` nor `wheelZoom`, its own range about the framed view.
// Otherwise the Babylon preview's rule: `zoom` (0..100, onto Babylon's radius divisor 1..2.8, over
// the 1.75 its default view uses) is the closest view, the wheel pulls back from it by `wheelZoom`,
// and `wheelStart` (0..100, 100 the closest, 50 by default) is where the view starts.
const num0 = (v) => (v === null || v === undefined || Number.isNaN(v) ? null : v);
export function zoomRange(o) {
  const zoom = num0(o.zoom), wheelZoom = num0(o.wheelZoom);
  if (zoom === null && wheelZoom === null) return { min: 1 / 1.5, max: 2, start: 1 };
  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
  const closest = zoom === null ? 1 : (1 + (1.8 * clamp(zoom, 0, 100)) / 100) / 1.75;
  const pullBack = clamp(wheelZoom ?? 1, 1, 100);
  const startFraction = (100 - clamp(num0(o.wheelStart) ?? 50, 0, 100)) / 100;
  return { min: closest / pullBack, max: closest, start: closest / (1 + (pullBack - 1) * startFraction) };
}

function cameraFrom(o) {
  const c = state.camera;
  // without auto-centre the view aims at the avatar's own origin, its feet, as the Unity renderer
  // aims at the subject's pivot; the offsets act from there
  const aimY = o.disableAutoCenter ? 0 : DEFAULT_TARGET_Y;
  c.target = [o.offsetX || 0, aimY + (o.offsetY || 0), o.offsetZ || 0];
  if (o.cameraX !== null || o.cameraY !== null || o.cameraZ !== null) {
    const dx = (o.cameraX || 0) - c.target[0], dy = (o.cameraY || 0) - c.target[1], dz = (o.cameraZ || 0) - c.target[2];
    c.radius = Math.max(0.5, Math.hypot(dx, dy, dz));
    c.alpha = Math.atan2(dx, dz);
    c.beta = Math.asin(dy / c.radius);
  } else {
    // `zoomScale` is extra zoom over the rest, as the Unity renderer takes it: 2 shows the subject
    // twice as large
    c.radius = DEFAULT_RADIUS / (o.zoomScale > 0 ? o.zoomScale : 1);
  }
  const mode = modeOf(o);
  if (PROFILE_MODES.includes(mode)) {
    // the Unity renderer's profile camera: 7 m in front, at a fixed 20 degree lens, looking 8.3
    // degrees down at 0.9 m (the jesus camera stands 0.32 m higher); never fitted to the avatar,
    // zoomed or panned, but the avatar still turns under a drag
    const lift = mode === 'jesus' ? 0.32 : 0;
    return {
      op: 'camera',
      alpha: c.alpha, beta: PROFILE_PITCH, radius: 7 / Math.cos(PROFILE_PITCH), fov: 20,
      target: [0, 0.903 + lift, 0],
      orthographic: o.projection === 'orthographic',
      autoRotate: false,
      interactive: o.camera !== 'static',
      dragSpeed: (0.5 * Math.PI) / 180,
      lockAlpha: !!o.lockAlpha,
      lockRadius: true,
      panning: false,
      fitSubject: false,
      fixed: true,
      reset: true,
    };
  }
  // the Unity renderer: input in marketplace and builder only; auto-rotation in marketplace, and
  // there only when no emote item plays; a builder avatar turns four times faster under the drag
  const interactive = o.camera !== 'static' && (mode === 'marketplace' || mode === 'builder');
  const zoom = zoomRange(o);
  return {
    op: 'camera',
    alpha: c.alpha, beta: c.beta, radius: c.radius, target: c.target,
    orthographic: o.projection === 'orthographic',
    autoRotate: autoRotateFor(o),
    autoRotateSpeed: o.autoRotateSpeed ?? 0.2,
    interactive,
    dragSpeed: ((mode === 'builder' ? 2 : 0.5) * Math.PI) / 180,
    lockAlpha: !!o.lockAlpha,
    lockBeta: !!o.lockBeta,
    lockRadius: !!o.lockRadius,
    panning: o.panning !== false,
    wheelPrecision: o.wheelPrecision ?? 100,
    zoomMin: zoom.min,
    zoomMax: zoom.max,
    zoomStart: zoom.start,
    autoCenter: !o.disableAutoCenter,
    // the builder view fits the avatar whatever its size, as the renderer's builder camera does
    fitSubject: mode === 'builder',
    fixed: false,
    fov: DEFAULT_FOV,
    reset: true,
  };
}

// the Unity renderer's auto-rotation: the avatar turns on its own in marketplace mode when no
// emote item plays; the item on its own turns in any interactive view
function autoRotateFor(o) {
  const mode = modeOf(o);
  const interactive = o.camera !== 'static' && (mode === 'marketplace' || mode === 'builder');
  if (!interactive || o.disableAutoRotate || o.lockAlpha) return false;
  return state.view === 'item' || (mode === 'marketplace' && !state.itemEmote);
}

// --- the view: the item on its own, or tried on the avatar ---
const FACIAL = ['eyes', 'eyebrows', 'mouth'];
// the view the user last picked, as the Unity renderer keeps it (its PlayerPrefs) for the next page
const AVATAR_SHOWN = 'PreviewAvatarShown';
const savedAvatarShown = () => { try { return localStorage.getItem(AVATAR_SHOWN) === '1'; } catch { return false; } };
const saveAvatarShown = (shown) => { try { localStorage.setItem(AVATAR_SHOWN, shown ? '1' : '0'); } catch { /* not kept */ } };

function showView(o, item) {
  state.view = item ? 'item' : 'avatar';
  command({ op: 'view', item });
  command({ op: 'camera', autoRotate: autoRotateFor(o) });
  ui.switcher(o);
}

// the Unity renderer's modes; an unknown or missing one is marketplace, as the wrapper sends
const MODES = ['marketplace', 'builder', 'profile', 'authentication', 'jesus', 'configurator'];
const modeOf = (o) => (MODES.includes(o.mode) ? o.mode : 'marketplace');
// the modes that show a profile's avatar as it is, from the Unity renderer's fixed camera
const PROFILE_MODES = ['profile', 'authentication', 'jesus'];
// that camera's pitch: its rotation in the Unity scene looks 8.26 degrees down
const PROFILE_PITCH = Math.asin(0.1436);

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
    const { outfit, itemEmote, solo, locked } = await resolveOutfit(o);
    state.itemEmote = itemEmote;
    state.solo = solo;
    state.locked = locked;
    state.lastBodyShape = outfit.bodyShape;
    // the view the page asks for (`type`), else the one last picked, else the item, as Unity opens
    const itemView = !!solo && (locked || (o.type === 'wearable' ? true : o.type === 'avatar' ? false : !savedAvatarShown()));
    state.view = itemView ? 'item' : 'avatar';
    ui.configure(o);
    command(cameraFrom(o));
    command({ op: 'boundaries', shown: !!o.showSceneBoundaries });
    // the Unity renderer's authentication platform; the auth page passes none of its own then
    command({ op: 'platform', shown: modeOf(o) === 'authentication' });
    // the Unity renderer's jesus mode (the download page): its particles off the avatar
    command({ op: 'effect', shown: modeOf(o) === 'jesus' });
    command({ op: 'background', color: o.disableBackground ? null : (o.background ? (o.background.startsWith('#') ? o.background : '#' + o.background) : DEFAULT_BACKGROUND) });
    command(outfit);
    command(solo
      ? {
          op: 'item', urn: solo.urn,
          // locked: shown on its own in the body shape it was made for
          bodyShape: locked ? solo.bodyShapes[0] : outfit.bodyShape,
          skin: outfit.skin, hair: outfit.hair, eyes: outfit.eyes,
          keepHead: FACIAL.includes(solo.category),
        }
      : { op: 'item', urn: null });
    showView(o, itemView);
    const peer = o.peerUrl || (o.env === 'dev' ? PEER.dev : PEER.prod);
    // jesus mode plays its own animation, looping, whatever emote the page names, as the renderer
    const urn = modeOf(o) === 'jesus' ? registerPose('particles', true) : itemEmote
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
    // a pixel size renders the view off screen at exactly that size; without one, the canvas as it
    // is, as the Unity renderer's wrapper asks for them
    getScreenshot: async (width, height) => {
      const sized = Number.isInteger(width) && width > 0 && Number.isInteger(height) && height > 0;
      const reply = await request('screenshot', sized ? { width, height } : {});
      return `data:image/png;base64,${reply.png}`;
    },
    getMetrics: async () => { const m = await request('metrics'); return { triangles: m.triangles, materials: m.materials, textures: m.textures, meshes: m.meshes, bodies: m.bodies, entities: m.entities }; },
    // Babylon's zoom buttons: the delta feeds an inertial offset that decays by 0.9 a frame, so the
    // radius moves ten times it (positive pulls closer), as the Unity renderer steps it
    changeZoom: async (delta) => { command({ op: 'zoomBy', radius: -10 * (Number(delta) || 0) }); },
    panCamera: async ({ x = 0, y = 0, z = 0 } = {}) => { const t = state.camera.target; state.camera.target = [t[0] + x, t[1] + y, t[2] + z]; sendCamera(); },
    changeCameraPosition: async ({ alpha = 0, beta = 0, radius = 0 } = {}) => {
      const c = state.camera;
      c.alpha += Number(alpha) || 0;
      c.beta += Number(beta) || 0;
      sendCamera();
      // the radius in Babylon's metres, within the zoom range
      if (radius) command({ op: 'zoomBy', radius: Number(radius) || 0 });
    },
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
  const showItem = $('show-item');
  const showAvatar = $('show-avatar');
  showItem?.addEventListener('click', () => {
    if (state.view === 'item') return;
    saveAvatarShown(false);
    showView(options(), true);
  });
  showAvatar?.addEventListener('click', () => {
    if (state.view === 'avatar' || state.locked) return;
    saveAvatarShown(true);
    showView(options(), false);
  });

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
    switcher(o) {
      body.classList.toggle('switcher', !!state.solo && !o.disableSwitcher);
      showItem?.classList.toggle('selected', state.view === 'item');
      showAvatar?.classList.toggle('selected', state.view === 'avatar');
      if (showAvatar) {
        showAvatar.disabled = !!state.locked;
        const shape = /female/i.test(state.lastBodyShape || '') ? 'Female' : 'Male';
        showAvatar.title = state.locked ? `This wearable does not have a ${shape} representation` : 'Try On';
      }
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
  } else if (data.type === 'bevy_effect' && data.payload && typeof data.payload === 'object') {
    // not part of the preview protocol: web/compare.html tunes the jesus particles with it
    command({ op: 'effect', ...data.payload });
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
