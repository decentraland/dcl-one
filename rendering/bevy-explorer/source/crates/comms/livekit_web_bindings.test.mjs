import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import {
  local_audio_track_set_enabled,
  local_audio_track_stop,
  remote_track_pan_and_volume,
  room_connect,
  room_drop,
} from './livekit_web_bindings.js';

test('capture enable toggles the underlying media track', () => {
  const mediaStreamTrack = { enabled: false };
  const track = { mediaStreamTrack };
  local_audio_track_set_enabled(track, true);
  assert.equal(mediaStreamTrack.enabled, true);
  local_audio_track_set_enabled(track, false);
  assert.equal(mediaStreamTrack.enabled, false);
});

test('stopping local capture disables the source before stopping the track', () => {
  const mediaStreamTrack = { enabled: true };
  const enabledAtStop = [];
  local_audio_track_stop({
    mediaStreamTrack,
    stop() { enabledAtStop.push(mediaStreamTrack.enabled); },
  });
  assert.equal(mediaStreamTrack.enabled, false);
  assert.deepEqual(enabledAtStop, [false]);
});

test('dropping a cancelled successful room disconnects it and releases listeners', () => {
  const operations = [];
  room_drop({
    disconnect() { operations.push('disconnect'); return Promise.resolve(); },
    removeAllListeners() { operations.push('release'); }
  });
  assert.deepEqual(operations, ['release', 'disconnect']);
});

test('dropping a room still releases listeners if asynchronous disconnect fails', async () => {
  let released = false;
  room_drop({
    disconnect() { return Promise.reject(new Error('already disconnected')); },
    removeAllListeners() { released = true; }
  });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(released, true);
});

test('spatial playback has a single gain and starts silent', async (t) => {
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const audiblePaths = new Set();
  const attachedElements = new Set();
  class Room {
    constructor(options) {
      this.options = options;
      this.listeners = new Map();
    }
    on(event, handler) { this.listeners.set(event, handler); }
    async connect() {}
  }
  const previous = [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream];
  t.after(() => {
    [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream] = previous;
  });
  globalThis.LivekitClient = { Room, RoomEvent: roomEvent };
  const gain = {
    gain: { value: 1 },
    connect() { audiblePaths.add(this.gain); },
    disconnect() { audiblePaths.delete(this.gain); },
  };
  const node = () => ({ connect() {}, disconnect() {} });
  globalThis.window = { AudioContext: class {
    destination = {};
    createMediaStreamSource() { return node(); }
    createStereoPanner() { return { ...node(), pan: { value: 0 } }; }
    createGain() { return gain; }
  } };
  globalThis.MediaStream = class {};
  const room = await room_connect('wss://room.example', 'token', { webAudioMix: true }, {}, () => {});
  const track = {
    sid: 'voice',
    kind: 'audio',
    mediaStreamTrack: {},
    attach() {
      // LiveKit 2.22.4 connects an independent Web Audio gain when webAudioMix is enabled.
      if (room.options.webAudioMix) audiblePaths.add({ value: 1 });
      const element = { volume: 1, muted: false, play() {}, pause() {} };
      attachedElements.add(element);
      return element;
    },
    detach(element) { attachedElements.delete(element); },
  };
  const speaker = { sid: 'speaker', identity: 'player' };
  const levels = () => [
    ...[...audiblePaths].map(path => path.value),
    ...[...attachedElements].filter(element => !element.muted && element.volume > 0).map(element => element.volume),
  ];
  room.listeners.get(roomEvent.TrackSubscribed)(track, {}, speaker);
  assert.deepEqual(levels(), [0]);
  remote_track_pan_and_volume(track, 0.5, 0.4);
  assert.deepEqual(levels(), [0.4]);
  remote_track_pan_and_volume(track, 0.5, 0);
  assert.deepEqual(levels(), [0]);
  room.listeners.get(roomEvent.TrackUnsubscribed)(track, {}, speaker);
  assert.deepEqual(levels(), []);
});

test('dropping one room releases its playback before disconnect settles', async (t) => {
  const { room_connect, room_drop, remote_track_pan_and_volume, remote_audio_track_set_volume } =
    await import('./livekit_web_bindings.js?room-drop');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  let finishDisconnect;
  const disconnect = new Promise(resolve => { finishDisconnect = resolve; });
  class Room extends EventEmitter {
    disconnectCalls = 0;
    async connect() {}
    async disconnect() { this.disconnectCalls++; await disconnect; }
  }
  const previous = [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream];
  t.after(() => {
    finishDisconnect();
    [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream] = previous;
  });
  globalThis.LivekitClient = { Room, RoomEvent: roomEvent };
  const gains = new Set();
  const attached = new Set();
  const node = () => ({ connect() {}, disconnect() {} });
  globalThis.window = { AudioContext: class {
    destination = {};
    createMediaStreamSource() { return node(); }
    createStereoPanner() { return { ...node(), pan: { value: 0 } }; }
    createGain() {
      const gain = { value: 1 };
      return { gain, connect() { gains.add(gain); }, disconnect() { gains.delete(gain); } };
    }
  } };
  globalThis.MediaStream = class {};
  function subscribe(room, identity, kind = 'audio') {
    const element = { volume: 1, play() {}, pause() {}, remove() {} };
    const track = {
      sid: 'track',
      kind,
      mediaStreamTrack: {},
      attach() { attached.add(element); return element; },
      detach(value) { attached.delete(value); },
      setVolume(volume) {
        assert.ok(attached.has(element), 'volume reached a detached element');
        element.volume = volume;
      },
    };
    room.emit(roomEvent.TrackSubscribed, track, {}, { sid: 'speaker', identity });
    return track;
  }
  const connect = () => room_connect('wss://room.example', 'token', {}, {}, () => {});
  const dropped = await connect();
  const other = await connect();
  const oldVoice = subscribe(dropped, 'speaker');
  const oldStream = subscribe(dropped, 'stream:show');
  subscribe(dropped, 'stream:show', 'video');
  const otherVoice = subscribe(other, 'speaker');
  remote_track_pan_and_volume(oldVoice, 0.5, 0.3);
  remote_track_pan_and_volume(otherVoice, 0.5, 0.7);
  remote_audio_track_set_volume(oldStream, 0.5);
  assert.equal(attached.size, 4);
  assert.deepEqual([...gains].map(gain => gain.value), [0.3, 0.7]);

  room_drop(dropped);
  room_drop(dropped);
  assert.equal(dropped.disconnectCalls, 1);
  assert.deepEqual([...gains].map(gain => gain.value), [0.7]);
  assert.equal(attached.size, 1);
  remote_track_pan_and_volume(oldVoice, 0.5, 1);
  remote_audio_track_set_volume(oldVoice, 1);
  remote_audio_track_set_volume(oldStream, 1);
  assert.deepEqual([...gains].map(gain => gain.value), [0.7]);
  assert.equal(attached.size, 1);
  room_drop(other);
  assert.equal(gains.size, 0);
  assert.equal(attached.size, 0);
});

test('repeat subscribe disposes the previous rig and streaming attachment', async (t) => {
  const { room_connect, room_drop, remote_track_pan_and_volume } =
    await import('./livekit_web_bindings.js?repeat-subscribe');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  class Room extends EventEmitter {
    async connect() {}
    async disconnect() {}
  }
  const previous = [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream];
  t.after(() => {
    [globalThis.LivekitClient, globalThis.window, globalThis.MediaStream] = previous;
  });
  globalThis.LivekitClient = { Room, RoomEvent: roomEvent };
  const connected = new Set();
  const node = (properties = {}) => ({
    ...properties,
    connect() { connected.add(this); },
    disconnect() { connected.delete(this); },
  });
  globalThis.window = { AudioContext: class {
    destination = {};
    createMediaStreamSource() { return node(); }
    createStereoPanner() { return node({ pan: { value: 0 } }); }
    createGain() { return node({ gain: { value: 1 } }); }
  } };
  globalThis.MediaStream = class {};
  const room = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  const attached = new Set();
  const audioTrack = sid => ({
    sid,
    kind: 'audio',
    mediaStreamTrack: {},
    attach() {
      const element = {
        volume: 1,
        playing: false,
        removed: false,
        play() { this.playing = true; },
        pause() { this.playing = false; },
        remove() { this.removed = true; },
      };
      attached.add(element);
      return element;
    },
    detach(element) { attached.delete(element); },
  });
  const voice = audioTrack('voice');
  const stream = audioTrack('stream');
  const subscribe = (track, identity) => {
    room.emit(roomEvent.TrackSubscribed, track, {}, { sid: identity, identity });
  };
  subscribe(voice, 'player');
  subscribe(stream, 'stream:show');
  const [oldVoice, oldStream] = attached;
  const oldNodes = [...connected];
  remote_track_pan_and_volume(voice, 0.5, 0.7);

  subscribe(voice, 'player');
  subscribe(stream, 'stream:show');
  assert.equal(attached.size, 2);
  assert.equal(attached.has(oldVoice), false);
  assert.equal(oldVoice.playing, false);
  assert.equal(attached.has(oldStream), false);
  assert.equal(oldStream.removed, true);
  assert.equal(connected.size, 3);
  assert.ok(oldNodes.every(node => !connected.has(node)));
  assert.deepEqual([...connected].flatMap(node => node.gain ? [node.gain.value] : []), [0]);

  room_drop(room);
  assert.equal(attached.size, 0);
  assert.equal(connected.size, 0);
});

const tick = () => new Promise(resolve => setImmediate(resolve));

function fakeDocument() {
  const listeners = new Map();
  return {
    listeners,
    addEventListener(event, listener) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(listener);
    },
    removeEventListener(event, listener) { listeners.get(event)?.delete(listener); },
    dispatch(event) { [...(listeners.get(event) ?? [])].forEach(listener => listener()); },
    events() { return [...listeners].filter(([, set]) => set.size > 0).map(([event]) => event).sort(); },
  };
}

const GESTURES = ['click', 'keydown', 'pointerdown', 'pointerup', 'touchend'];

function withPageGlobals(t, globals) {
  const names = Object.keys(globals);
  const previous = names.map(name => globalThis[name]);
  t.after(() => names.forEach((name, i) => { globalThis[name] = previous[i]; }));
  Object.assign(globalThis, globals);
}

// The browser's autoplay policy: play() of an audible element is refused until activation.
function fakePage() {
  const page = { activated: false, plays: 0 };
  page.element = () => {
    const element = {
      muted: false,
      volume: 1,
      paused: true,
      play() {
        page.plays++;
        if (!page.activated && !element.muted) {
          return Promise.reject(Object.assign(new Error('refused'), { name: 'NotAllowedError' }));
        }
        element.paused = false;
        return Promise.resolve();
      },
      pause() { element.paused = true; },
      remove() {},
    };
    return element;
  };
  return page;
}

// LiveKit's Room, including what startAudio() does with webAudioMix off: unmute and play every
// attached remote audio element. The bindings must not call it (the voice rigs stay muted).
function fakeRoomClass(attached) {
  return class Room extends EventEmitter {
    name = 'scene-room';
    async connect() {}
    async disconnect() {}
    async startAudio() {
      for (const element of attached) {
        element.muted = false;
        await element.play();
      }
    }
  };
}

function remoteAudio(sid, page, attached) {
  return {
    sid,
    kind: 'audio',
    mediaStreamTrack: {},
    attach() {
      const element = page.element();
      attached.add(element);
      element.play().catch(() => {});
      return element;
    },
    detach(element) { attached.delete(element); },
  };
}

function suspendedAudioContextClass(contexts) {
  const node = () => ({ connect() {}, disconnect() {} });
  return class AudioContext extends EventTarget {
    state = 'suspended';
    destination = {};
    resumeCalls = 0;
    constructor() { super(); contexts.push(this); }
    setState(state) { this.state = state; this.dispatchEvent(new Event('statechange')); }
    createMediaStreamSource() { return node(); }
    createStereoPanner() { return { ...node(), pan: { value: 0 } }; }
    createGain() { return { ...node(), gain: { value: 1 } }; }
    async resume() {
      this.resumeCalls++;
      if (page_activated()) this.setState('running');
      else throw Object.assign(new Error('refused'), { name: 'NotAllowedError' });
    }
  };
}
let page_activated = () => true;

test('a refused stream plays on a gesture; voice rig elements stay muted', async (t) => {
  const { room_connect } = await import('./livekit_web_bindings.js?stream-gesture');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const attached = new Set();
  const page = fakePage();
  const contexts = [];
  page_activated = () => page.activated;
  const document = fakeDocument();
  withPageGlobals(t, {
    LivekitClient: { Room: fakeRoomClass(attached), RoomEvent: roomEvent },
    window: { AudioContext: suspendedAudioContextClass(contexts) },
    MediaStream: class {},
    document,
  });

  const room = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  const stream = remoteAudio('stream', page, attached);
  const voice = remoteAudio('voice', page, attached);
  room.emit(roomEvent.TrackSubscribed, stream, {}, { sid: 'ingress', identity: 'abc-streamer' });
  room.emit(roomEvent.TrackSubscribed, voice, {}, { sid: 'player', identity: 'player' });
  await tick();
  assert.equal(stream.audioElement.paused, true);
  assert.deepEqual(document.events(), GESTURES);

  page.activated = true;
  document.dispatch('pointerup');
  await tick();
  assert.equal(stream.audioElement.paused, false, 'the stream element plays');
  assert.equal(voice.trackRig.audioElement.muted, true, 'the rig element stays muted');
  assert.equal(contexts[0].state, 'running');
  assert.deepEqual(document.events(), [], 'nothing waits: the listener is removed');

  const plays = page.plays;
  document.dispatch('click');
  await tick();
  assert.equal(page.plays, plays);
});

test('a gesture without activation keeps the listener; a later one succeeds', async (t) => {
  const { room_connect } = await import('./livekit_web_bindings.js?no-activation');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const attached = new Set();
  const page = fakePage();
  const document = fakeDocument();
  withPageGlobals(t, { LivekitClient: { Room: fakeRoomClass(attached), RoomEvent: roomEvent }, document });

  const room = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  const stream = remoteAudio('stream', page, attached);
  room.emit(roomEvent.TrackSubscribed, stream, {}, { sid: 'ingress', identity: 'abc-streamer' });
  await tick();

  // a touch pointerdown (or Escape) grants no activation: play() is refused again
  document.dispatch('pointerdown');
  await tick();
  assert.equal(stream.audioElement.paused, true);
  assert.deepEqual(document.events(), GESTURES);

  page.activated = true;
  document.dispatch('touchend');
  await tick();
  assert.equal(stream.audioElement.paused, false);
  assert.deepEqual(document.events(), []);
});

test('the voice context listener follows its statechange until running', async (t) => {
  const { room_connect } = await import('./livekit_web_bindings.js?context-state');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const attached = new Set();
  const page = fakePage();
  const contexts = [];
  page_activated = () => page.activated;
  const document = fakeDocument();
  withPageGlobals(t, {
    LivekitClient: { Room: fakeRoomClass(attached), RoomEvent: roomEvent },
    window: { AudioContext: suspendedAudioContextClass(contexts) },
    MediaStream: class {},
    document,
  });

  const room = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  room.emit(roomEvent.TrackSubscribed, remoteAudio('a', page, attached), {}, { sid: 'p1', identity: 'player-a' });
  room.emit(roomEvent.TrackSubscribed, remoteAudio('b', page, attached), {}, { sid: 'p2', identity: 'player-b' });
  assert.equal(contexts.length, 1);
  assert.deepEqual(document.events(), GESTURES);

  document.dispatch('keydown');
  await tick();
  assert.equal(contexts[0].resumeCalls, 1);
  assert.deepEqual(document.events(), GESTURES, 'a refused resume keeps the listener');

  page.activated = true;
  document.dispatch('click');
  await tick();
  assert.equal(contexts[0].state, 'running');
  assert.deepEqual(document.events(), []);

  // the browser suspends the context later (an interruption): no new rig is needed to re-arm
  contexts[0].setState('suspended');
  assert.deepEqual(document.events(), GESTURES);
  document.dispatch('pointerdown');
  await tick();
  assert.equal(contexts[0].state, 'running');
  assert.deepEqual(document.events(), []);
});

test('a failed connect releases a refused stream and its listener', async (t) => {
  const { room_connect } = await import('./livekit_web_bindings.js?failed-connect');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const attached = new Set();
  const page = fakePage();
  const document = fakeDocument();
  let stream;
  class Room extends fakeRoomClass(attached) {
    async connect() {
      stream = remoteAudio('stream', page, attached);
      this.emit(roomEvent.TrackSubscribed, stream, {}, { sid: 'ingress', identity: 'abc-streamer' });
      await tick();
      throw new Error('join failed');
    }
  }
  withPageGlobals(t, { LivekitClient: { Room, RoomEvent: roomEvent }, document });

  await assert.rejects(room_connect('wss://room.example', 'token', {}, {}, () => {}), /join failed/);
  assert.deepEqual(document.events(), []);
  assert.equal(attached.size, 0);
  const plays = page.plays;
  page.activated = true;
  document.dispatch('click');
  await tick();
  assert.equal(page.plays, plays);
});

test('a dropped room is not retried, even by a refusal that settles after the drop', async (t) => {
  const { room_connect, room_drop } = await import('./livekit_web_bindings.js?dropped');
  const roomEvent = new Proxy({}, { get: (_, name) => name });
  const attached = new Set();
  const page = fakePage();
  const document = fakeDocument();
  withPageGlobals(t, { LivekitClient: { Room: fakeRoomClass(attached), RoomEvent: roomEvent }, document });

  const room = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  room.emit(roomEvent.TrackSubscribed, remoteAudio('stream', page, attached), {}, { sid: 'ingress', identity: 'abc-streamer' });
  // dropped before the first play() settles
  room_drop(room);
  await tick();
  assert.deepEqual(document.events(), []);

  const other = await room_connect('wss://room.example', 'token', {}, {}, () => {});
  other.emit(roomEvent.TrackSubscribed, remoteAudio('stream2', page, attached), {}, { sid: 'ingress', identity: 'def-streamer' });
  await tick();
  assert.deepEqual(document.events(), GESTURES);
  room_drop(other);
  assert.deepEqual(document.events(), []);
  const plays = page.plays;
  page.activated = true;
  document.dispatch('click');
  await tick();
  assert.equal(page.plays, plays, 'no play() after the drop');
});
