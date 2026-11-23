import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Entity } from '@dcl/ecs';
import { copyToClipboard } from '~system/RestrictedActions';
import { init, step } from '@dcl/sdk/cast';
import type { PBRealmInfo } from '~system/Runtime';
import type * as SdkCast from '@dcl/sdk/cast';
import type { Cast, Effect, Event, Fetched, State } from '@dcl/sdk/cast';
import { clearAdminConfig, setAdminConfig } from '../src/admin-toolkit-ui/config';
import { icon } from '../src/admin-toolkit-ui/icons';
import { docsUrl } from '../src/admin-toolkit-ui/fetch-utils';
import { expiresIn } from '../src/admin-toolkit-ui/cast';
import { urlAction } from '../src/admin-toolkit-ui/VideoControl/utils';

// The panels read one handle. Here it is the library's own reducer; the test plays the explorer and
// the server: it reads the effects the reducer returns and sends the answers.
const T0 = 1_800_000_000_000;
const DAY = 86_400_000;
const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const LATE = '0x00000000000000000000000000000000000000cc';
const VISITOR = '0x00000000000000000000000000000000000000dd';
const ADMINS = [ME, OTHER, LATE].map(admin => ({ admin, name: admin, canBeRemoved: true }));
const BOT = 'presentation-bot:1';
const GATEKEEPER = 'https://node.test/comms-gatekeeper';

let cast: ReturnType<typeof follow>;

function follow() {
  let now = T0;
  const [start, first] = init('owner', now);
  let state = start;
  const effects: Effect[] = [...first];
  const send = (event: Event) => {
    const [next, more] = step(state, event);
    state = next;
    effects.push(...more);
  };
  const last = (tag: Effect['tag'], of?: Fetched) =>
    effects.filter(f => f.tag === tag && (!of || (f.tag === 'fetch' && f.of === of))).pop() as
      | (Effect & { seq: number })
      | undefined;
  send({ tag: 'realm', baseUrl: 'https://node.test', preview: false, address: ME });
  return {
    get state(): State {
      return state;
    },
    command: (cmd: never) => send({ tag: 'command', cmd }),
    tick: (ms = 0) => send({ tag: 'tick', now: (now += ms) }),
    answer: (of: Fetched, status: number, body: object = {}) =>
      send({ tag: 'answer', seq: last('fetch', of)!.seq, of, status, body: JSON.stringify(body) }),
    tracks: (identities: string[]) =>
      send({
        tag: 'tracks',
        seq: last('tracks')!.seq,
        tracks: identities.map(identity => ({ identity, trackSid: 'TR_1', sourceType: 2 })),
      }),
    packets: (sender: string, data: object) =>
      send({
        tag: 'packets',
        seq: last('packets')!.seq,
        packets: [{ sender, data: JSON.stringify(data) }],
      }),
    fetches: () => effects.flatMap(f => (f.tag === 'fetch' ? [`${f.method} ${f.url}`] : [])),
    published: () => effects.flatMap(f => (f.tag === 'publish' ? [JSON.parse(f.data)] : [])),
  };
}

vi.mock('@dcl/sdk/cast', async original => ({
  ...(await original<typeof SdkCast>()),
  openCast: (): Partial<Cast<'owner'>> => ({
    get state() {
      return cast.state;
    },
    send: cmd => cast.command(cmd as never),
    onChange: () => undefined,
  }),
}));

// The scene's text comms: a message that one client sends reaches every other client with the
// sender's address. `from` is the client that acts now.
type Handler = (payload: unknown, sender: string) => void;
type Peer = { address: string; handlers: Map<string, Handler> };
const net = vi.hoisted(() => ({ from: undefined as Peer | undefined, peers: [] as Peer[] }));
function as<T>(peer: Peer, act: () => T): T {
  const before = net.from;
  net.from = peer;
  try {
    return act();
  } finally {
    net.from = before;
  }
}
vi.mock('@dcl/sdk/message-bus', () => ({
  MessageBus: class {
    on = (type: string, handler: Handler) => net.from!.handlers.set(type, handler);
  },
}));
const fetched = vi.hoisted(() => [] as string[]);
// One stub file stands for every `~system` module, so one mock holds all of them.
vi.mock('~system/RestrictedActions', async original => ({
  ...(await original<object>()),
  copyToClipboard: vi.fn(),
  openExternalUrl: vi.fn(),
  signedFetch: async ({ url }: { url: string }) => {
    fetched.push(url);
    return { ok: true, body: JSON.stringify(url.endsWith('/scene-admin') ? ADMINS : {}) };
  },
  send: async ({ message }: { message: string }) => {
    const { message: type, payload } = JSON.parse(message);
    const from = net.from!;
    for (const to of net.peers)
      if (to !== from) as(to, () => to.handlers.get(type)?.(payload, from.address));
  },
}));

// A react-ecs tree as the panel returns it. A component with hooks (a button) stays a leaf: its props
// say what it shows and what a click does. React logs each hook it refuses.
vi.spyOn(console, 'error').mockImplementation(() => undefined);
type Node = { type: unknown; props: Record<string, any>; parent?: Node };
function nodes(tree: unknown, parent?: Node): Node[] {
  if (Array.isArray(tree)) return tree.flatMap(child => nodes(child, parent));
  if (!tree || typeof tree !== 'object') return [];
  const node: Node = { ...(tree as Node), parent };
  let inner: unknown = node.props.children;
  if (typeof node.type === 'function') {
    try {
      inner = node.type(node.props);
    } catch {
      inner = undefined;
    }
  }
  return [node, ...nodes(inner, node)];
}
const plain = (text: unknown) => (typeof text === 'string' ? text.replace(/<\/?b>/g, '') : '');
const said = ({ props }: Node) =>
  [props.value, props.label, props.title, props.text, props.uiText?.value]
    .map(plain)
    .filter(Boolean);
const texts = (tree: unknown) => nodes(tree).flatMap(said);
function click(tree: unknown, what: string, who: Peer = me) {
  const node = nodes(tree).find(
    n => n.props.id === what || said(n).includes(what) || n.props.name === what,
  );
  if (!node) throw new Error(`nothing to click: ${what}`);
  const on = (n: Node) => n.props.onClick ?? n.props.onMouseDown ?? n.props.onCopy;
  as(who, on(node) ?? on(node.parent!));
}

/**
 * One player's copy of the scene: its own modules (view state, message bus), engine and screen. It
 * starts as the item does: it reads the admins, then joins the bus.
 */
async function join(address: string) {
  vi.resetModules();
  const { Engine } = await import('@dcl/ecs');
  const { createComponents } = await import('../src/definitions');
  const { getExplorerComponents } = await import('../src/components');
  const { setAdminConfig } = await import('../src/admin-toolkit-ui/config');
  const { state } = await import('../src/admin-toolkit-ui/store');
  const { initializeAdminData } = await import('../src/admin-toolkit-ui');
  const { getAdminMessageBus } = await import('../src/admin-toolkit-ui/admin-message-bus');
  const { LiveStream } = await import('../src/admin-toolkit-ui/VideoControl/LiveStream');
  const { default: DclCast } = await import('../src/admin-toolkit-ui/VideoControl/DclCast');
  const engine = Engine();
  createComponents(engine);
  const { VideoPlayer } = getExplorerComponents(engine);
  const entity = engine.addEntity();
  VideoPlayer.create(entity, { src: '' });
  const videoPlayers = [{ entity, customName: 'Screen' }];
  setAdminConfig({ videoControl: { isEnabled: true, videoPlayers } } as never, engine.addEntity());
  const peer: Peer = { address, handlers: new Map() };
  net.peers.push(peer);
  net.from = peer;
  await initializeAdminData(engine);
  net.from = undefined;
  const video = () => VideoPlayer.getOrNull(entity) ?? undefined;
  return Object.assign(peer, {
    src: () => video()?.src,
    setVolume: (volume: number) =>
      as(peer, () => getAdminMessageBus().emitSetVideo(entity, { volume })),
    stream: () => LiveStream({ engine, entity, video: video() }),
    room: () => DclCast({ engine, state, entity, video: video() }),
  });
}
let me: Awaited<ReturnType<typeof join>>;
const stream = () => me.stream();
const room = () => me.room();

const key = (name: string, endsAt = T0 + 4 * DAY) => ({
  streaming_key: name,
  ends_at: endsAt,
  streaming_url: 'rtmp://node.test:1935/x',
});
const links = (name: string) => ({
  streamingKey: name,
  expiresAt: new Date(T0 + 4 * DAY).toISOString(),
  streamLink: `https://node.test/cast/s/${name}`,
  watcherLink: 'https://node.test/cast/w/place',
});

beforeEach(async () => {
  cast = follow();
  net.peers.length = fetched.length = 0;
  me = await join(ME);
  vi.mocked(copyToClipboard).mockClear();
});

function withKey(name = 'KEY-1') {
  cast.tick();
  cast.answer('key', 200, key(name));
}

describe('when the place has a stream key', () => {
  beforeEach(() => withKey());

  it('should show the RTMP server and hide the key', () => {
    expect(texts(stream())).toContain('rtmp://node.test:1935/x');
    expect(texts(stream())).not.toContain('KEY-1');
  });

  it('should show and copy the key it holds without a request', () => {
    const before = cast.fetches().length;
    click(stream(), 'eye');
    expect(texts(stream())).toContain('KEY-1');
    click(stream(), 'video_control_copy_stream_key');
    expect(copyToClipboard).toHaveBeenCalledWith({ text: 'KEY-1' });
    expect(cast.fetches()).toHaveLength(before);
  });

  it('should give one expiry text to the Stream and the Cast sections', () => {
    cast.tick(1_000);
    expect(texts(stream())).toContain('Stream expires in 4 days');
    room();
    cast.answer('link', 200, links('KEY-1'));
    expect(texts(room())).toContain('Expires in 4 days');
  });
});

describe('when the key is revoked while the panel is open', () => {
  beforeEach(() => {
    withKey();
    cast.tick(60_000);
    cast.answer('key', 404, { error: 'no streaming access' });
  });

  it('should say so and offer a new key', () => {
    expect(texts(stream())).toContain('The stream key was revoked.');
    expect(texts(stream())).toContain('Get Stream Key');
    expect(texts(stream())).not.toContain('rtmp://node.test:1935/x');
  });

  it('should not make a new key when the Cast section opens', () => {
    const before = cast.fetches().length;
    expect(texts(room())).toContain('Get Cast links');
    expect(cast.fetches()).toHaveLength(before);
  });
});

describe('when another admin makes a new key', () => {
  beforeEach(() => {
    withKey();
    cast.tick(60_000);
    cast.answer('key', 200, key('KEY-2'));
  });

  it('should say so and copy the new key', () => {
    expect(texts(stream())).toContain('Another admin made a new stream key.');
    click(stream(), 'video_control_copy_stream_key');
    expect(copyToClipboard).toHaveBeenCalledWith({ text: 'KEY-2' });
  });
});

describe('when the key reaches its end', () => {
  beforeEach(() => {
    cast.tick();
    cast.answer('key', 200, key('KEY-1', T0 + 3_600_000));
  });

  it('should count the last day down', () => {
    cast.tick(1_000);
    expect(texts(stream())).toContain('Stream expires in 00:59:59');
  });

  it('should say that the key expired and offer a new one', () => {
    cast.tick(3_600_000);
    expect(texts(stream())).toContain('The stream key expired.');
    expect(texts(stream())).toContain('Get Stream Key');
  });
});

describe('when the admin revokes the key', () => {
  beforeEach(() => withKey());

  it('should ask first', () => {
    const before = cast.fetches().length;
    click(stream(), 'Revoke stream key');
    expect(texts(stream())).toContain('Revoke the stream key?');
    click(stream(), 'Cancel');
    expect(cast.fetches()).toHaveLength(before);
    expect(texts(stream())).toContain('rtmp://node.test:1935/x');
  });

  it('should send one revoke and then offer a new key', () => {
    click(stream(), 'Revoke stream key');
    click(stream(), 'Revoke');
    expect(cast.fetches().pop()).toBe(`DELETE ${GATEKEEPER}/scene-stream-access`);
    cast.answer('revoke', 204);
    expect(texts(stream())).toContain('Get Stream Key');
  });

  it('should show why the server refused', () => {
    click(stream(), 'Revoke stream key');
    click(stream(), 'Revoke');
    cast.answer('revoke', 403, { error: 'Only the owner can revoke' });
    expect(texts(stream())).toContain('Only the owner can revoke');
  });
});

describe('when the Cast section opens', () => {
  it('should ask for the links of the key once', () => {
    withKey();
    room();
    room();
    expect(cast.fetches().filter(f => f.includes('generate-stream-link'))).toHaveLength(1);
    cast.answer('link', 200, links('KEY-1'));
    click(room(), 'dcl_cast_copy_stream_link');
    expect(copyToClipboard).toHaveBeenCalledWith({ text: 'https://node.test/cast/s/KEY-1' });
  });

  it('should ask before it resets the room, then read the new links', () => {
    withKey();
    room();
    cast.answer('link', 200, links('KEY-1'));
    const before = cast.fetches().length;
    click(room(), 'Reset room');
    expect(texts(room())).toContain('Reset the Cast room?');
    expect(cast.fetches()).toHaveLength(before);
    click(room(), 'Reset');
    expect(cast.fetches().pop()).toBe(`PUT ${GATEKEEPER}/scene-stream-access`);
    cast.answer('reset', 200, key('KEY-2'));
    room();
    expect(cast.fetches().pop()).toBe(`GET ${GATEKEEPER}/cast/generate-stream-link`);
  });
});

describe('when a presentation is on', () => {
  beforeEach(() => {
    withKey();
    room();
    cast.answer('link', 200, links('KEY-1'));
    cast.tracks([BOT]);
  });

  it('should show it with a stop control while no slide state arrives', () => {
    expect(texts(room())).toContain('Presenting');
    expect(texts(room()).join()).not.toMatch(/Slide \d/);
    click(room(), 'dcl_cast_stop');
    expect(cast.published()).toContainEqual({ type: 'presentation:stop' });
  });

  describe('and this admin is a presenter', () => {
    beforeEach(() => {
      cast.tick(1_000);
      cast.answer('presenters', 200, { presenters: [ME] });
      cast.tick(1_000);
    });

    it('should show the slide controls when the bot states the slides', () => {
      cast.packets(BOT, { type: 'presentation:state', id: 'p', currentSlide: 1, slideCount: 6 });
      expect(texts(room())).toContain('Slide 2 of 6');
      click(room(), 'dcl_cast_next');
      expect(cast.published()).toContainEqual({ type: 'presentation:navigate', action: 'next' });
    });

    describe('and the slide has a video', () => {
      const slide = { type: 'presentation:state', id: 'p', slideCount: 6, slideVideos: [{}] };

      it('should offer the play while the bot does not say that the video plays', () => {
        cast.packets(BOT, { ...slide, videoState: 'idle' });
        expect(texts(room())).not.toContain('Pause video');
        click(room(), 'Play video');
        expect(cast.published()).toContainEqual({ type: 'presentation:video:play', videoIndex: 0 });
      });

      it('should offer the pause while the bot says that the video plays', () => {
        cast.packets(BOT, { ...slide, videoState: 'playing' });
        expect(texts(room())).not.toContain('Play video');
        click(room(), 'Pause video');
        expect(cast.published()).toContainEqual({ type: 'presentation:video:pause' });
      });
    });

    it('should not take the slides from a wallet', () => {
      cast.packets(ME, { type: 'presentation:state', id: 'p', currentSlide: 1, slideCount: 6 });
      expect(texts(room()).join()).not.toMatch(/Slide \d/);
      expect(texts(room())).toContain('Presenting');
    });
  });
});

describe('when two admins are in the scene', () => {
  let other: typeof me;
  const said = (who: typeof me) => [...texts(who.stream()), ...texts(who.room())];
  beforeEach(async () => {
    other = await join(OTHER);
    withKey();
    room();
    cast.answer('link', 200, links('KEY-1'));
  });

  it('should show the stream that one of them activates as active on both panels', () => {
    click(other.stream(), 'video_control_stream_activate', other);
    expect(me.src()).toBe('livekit-video://current-stream');
    for (const who of [me, other]) {
      expect(said(who)).toContain('Deactivate');
      expect(said(who)).toContain('Activate room');
    }
  });

  it('should show the Cast room that one of them activates as active on both panels', () => {
    click(me.stream(), 'video_control_stream_activate');
    click(other.room(), 'Activate room', other);
    for (const who of [me, other]) {
      expect(said(who)).toContain('Activate');
      expect(said(who)).toContain('Deactivate room');
    }
  });

  it('should show no active source on both panels when one of them deactivates', () => {
    click(me.room(), 'Activate room');
    click(other.room(), 'Deactivate room', other);
    expect(me.src()).toBe('');
    for (const who of [me, other]) {
      expect(said(who)).toContain('Activate');
      expect(said(who)).toContain('Activate room');
    }
  });

  it('should keep the active source when an admin changes the volume only', () => {
    click(me.room(), 'Activate room');
    other.setVolume(0.5);
    for (const who of [me, other]) expect(said(who)).toContain('Deactivate room');
  });

  it('should show the stream as active on the panel of an admin who joins later', async () => {
    click(other.stream(), 'video_control_stream_activate', other);
    const late = await join(LATE);
    expect(late.src()).toBe('livekit-video://current-stream');
    expect(said(late)).toContain('Deactivate');
    expect(said(late)).toContain('Activate room');
  });

  it('should show the Cast room as active on the panel of an admin who joins later', async () => {
    click(other.room(), 'Activate room', other);
    const late = await join(LATE);
    expect(said(late)).toContain('Activate');
    expect(said(late)).toContain('Deactivate room');
  });
});

describe('when a visitor enters the scene', () => {
  it('should read the admins and not ask for the ban list', async () => {
    fetched.length = 0;
    await join(VISITOR);
    expect(fetched).toEqual(['https://comms-gatekeeper.decentraland.org/scene-admin']);
  });
});

describe('the control of the URL section', () => {
  const playing = 'https://node.test/a.m3u8';

  it("should show the screen's state while the admin typed nothing", () => {
    expect(urlAction(playing, '')).toBe('Deactivate');
    expect(urlAction(playing, playing)).toBe('Deactivate');
    expect(urlAction('', '')).toBe('Activate');
    expect(urlAction('livekit-video://current-stream', playing)).toBe('Activate');
  });

  it('should offer the update for another URL', () => {
    expect(urlAction(playing, 'https://node.test/b.m3u8')).toBe('Update');
  });
});

describe('expiresIn', () => {
  it('should give the nearest day from one day up and the clock below', () => {
    expect(expiresIn(T0 + 4 * DAY, T0 + 1_000)).toBe('4 days');
    expect(expiresIn(T0 + DAY, T0)).toBe('1 day');
    expect(expiresIn(T0 + DAY, T0 + 1_000)).toBe('23:59:59');
    expect(expiresIn(T0, T0 + 1_000)).toBe('00:00:00');
  });
});

describe('the hosts of the panel', () => {
  const realm = (baseUrl: string) => ({ baseUrl, networkId: 1 }) as PBRealmInfo;
  const help = '/creator/scene-editor/live-ops/live-streaming';

  it("should take the icons from the item's own folder in the scene", () => {
    setAdminConfig({} as never, 513 as Entity, 'assets/asset-packs/admin_tools');
    expect(icon('eye')).toBe('assets/asset-packs/admin_tools/assets/icons/reskin/eye.png');
    clearAdminConfig(513 as Entity);
  });

  it('should keep the item bucket for an item that names no folder', () => {
    expect(icon('eye')).toBe(
      'https://builder-items.decentraland.org/admin_toolkit/assets/icons/reskin/eye.png',
    );
  });

  it("should open the docs of the realm's own deployment", () => {
    expect(docsUrl(help, realm('https://node.test/content'))).toBe(`https://node.test/docs${help}`);
    expect(docsUrl(help, realm('https://peer.decentraland.zone'))).toBe(
      `https://docs.decentraland.org${help}`,
    );
    expect(docsUrl(help, undefined)).toBe(`https://docs.decentraland.org${help}`);
  });
});
