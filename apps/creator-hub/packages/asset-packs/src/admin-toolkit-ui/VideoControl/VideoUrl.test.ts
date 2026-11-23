import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Engine } from '@dcl/ecs';
import type { Entity, IEngine } from '@dcl/ecs';
import ReactEcs, { createReactBasedUiSystem } from '@dcl/react-ecs';
import { createComponents, getComponents } from '../../definitions';
import { getExplorerComponents } from '../../components';
import { state } from '../store';
import { selectVideoPlayer, selectVideoSubTab } from '../actions';
import { COLORS } from '../theme';
import { VideoControl } from './index';

const mocks = vi.hoisted(() => ({
  emitSetVideo: vi.fn(),
  src: (_screen: unknown): string | undefined => undefined,
}));

// The panel publishes source changes through the admin message bus, which needs the scene's comms.
// The bus also tells the panel what a screen plays: here that is the screen's own address.
vi.mock('../admin-message-bus', () => ({
  getAdminMessageBus: () => ({
    emitSetVideo: mocks.emitSetVideo,
    source: (screen: unknown) => {
      const src = mocks.src(screen);
      if (!src) return undefined;
      return src.startsWith('livekit-video://') ? 'live' : 'video-url';
    },
  }),
}));
// The Cast state needs the scene's runtime; no presentation runs in these tests.
vi.mock('../cast', () => ({ cast: () => ({ state: { tag: 'idle' } }) }));
// The DCL Cast and Stream panels call remote services; these tests only switch to them and back.
vi.mock('./DclCast', () => ({ default: () => null }));
vi.mock('./LiveStream', () => ({ LiveStream: () => null }));

const RUNNING_URL = 'https://example.com/live/running.m3u8';
const OTHER_URL = 'https://example.com/live/other.m3u8';
const PRIMARY_LABELS = ['Activate', 'Update', 'Deactivate'];

type PrimaryControl = { label: string; enabled: boolean; button: Entity };
type Frame = { primary: PrimaryControl | undefined; input: string | undefined };

// Renders the real VideoControl with the installed @dcl/react-ecs renderer on a fresh engine and
// reads back what the scene would show: the primary URL button and the URL input, frame by frame.
function createPanel(screens: { src: string }[]) {
  const engine: IEngine = Engine();
  createComponents(engine);
  const { AdminTools } = getComponents(engine);
  const { VideoPlayer, UiInput, UiInputResult, UiText, UiTransform } =
    getExplorerComponents(engine);
  const screenEntities = screens.map(({ src }) => {
    const screen = engine.addEntity();
    VideoPlayer.create(screen, { src, playing: true });
    return screen;
  });
  mocks.src = screen => VideoPlayer.getOrNull(screen as Entity)?.src;
  AdminTools.create(engine.addEntity(), {
    videoControl: {
      isEnabled: true,
      disableVideoPlayersSound: false,
      showAuthorOnVideoPlayers: false,
      linkAllVideoPlayers: false,
      videoPlayers: screenEntities.map((entity, i) => ({ entity, customName: `Screen ${i}` })),
    },
  } as never);

  // Pointer clicks reach the UI through the pointer events system; keep the callbacks so tests can
  // press a button the way the renderer would.
  const pointerDown = new Map<Entity, () => void>();
  const pointerEvents = {
    onPointerDown: ({ entity }: { entity: Entity }, cb: () => void) => pointerDown.set(entity, cb),
    removeOnPointerDown: (entity: Entity) => pointerDown.delete(entity),
    onPointerUp: () => {},
    removeOnPointerUp: () => {},
    onPointerHoverEnter: () => {},
    removeOnPointerHoverEnter: () => {},
    onPointerHoverLeave: () => {},
    removeOnPointerHoverLeave: () => {},
  };
  const ui = createReactBasedUiSystem(engine, pointerEvents as never);
  ui.setUiRenderer(() =>
    ReactEcs.createElement(VideoControl, { engine, state, playerAddress: undefined }),
  );

  const readPrimary = (): PrimaryControl | undefined => {
    for (const [entity, text] of engine.getEntitiesWith(UiText)) {
      const label = PRIMARY_LABELS.find(l => text.value === `<b>${l}</b>`);
      if (!label) continue;
      const button = UiTransform.get(entity).parent as Entity;
      const disabled = JSON.stringify(text.color) === JSON.stringify(COLORS.textDisabled);
      return { label, enabled: !disabled, button };
    }
    return undefined;
  };
  const readInput = (): { entity: Entity; value: string | undefined } | undefined => {
    for (const [entity, input] of engine.getEntitiesWith(UiInput)) {
      return { entity, value: input.value };
    }
    return undefined;
  };

  // Recorded by a system that runs right after the UI system in the same tick, so each entry is what
  // a frame rendered before any deferred effect gets the chance to correct it.
  const frames: Frame[] = [];
  engine.addSystem(() => frames.push({ primary: readPrimary(), input: readInput()?.value }), -1);

  async function frame(): Promise<Frame> {
    await engine.update(1);
    return frames[frames.length - 1];
  }
  async function settle() {
    for (let i = 0; i < 3; i++) await frame();
    return frames[frames.length - 1];
  }
  function press(control: PrimaryControl | undefined) {
    const cb = control && pointerDown.get(control.button);
    if (!cb) throw new Error('primary URL button has no click handler');
    cb();
  }
  function type(value: string) {
    const input = readInput();
    if (!input) throw new Error('URL input is not rendered');
    // The renderer reports typed text as a UiInputResult change on the input entity.
    (
      UiInputResult as unknown as { __onChangeCallbacks: (e: Entity, v: unknown) => void }
    ).__onChangeCallbacks(input.entity, { value, isSubmit: false });
  }
  function setSource(screenIndex: number, src: string) {
    VideoPlayer.getMutable(screenEntities[screenIndex]).src = src;
  }

  return { engine, ui, screenEntities, frame, settle, press, type, setSource };
}

function resetPanelState() {
  state.videoControl.selectedVideoPlayer = undefined;
  state.videoControl.selectedStream = undefined;
  state.videoControl.selectedTab = undefined;
}

describe('when an admin returns to the URL source of a screen', () => {
  let panel: ReturnType<typeof createPanel>;

  beforeEach(() => {
    resetPanelState();
    mocks.emitSetVideo.mockReset();
  });

  afterEach(() => {
    panel?.ui.destroy();
    resetPanelState();
  });

  describe('and the screen is already playing a video URL', () => {
    beforeEach(async () => {
      panel = createPanel([{ src: RUNNING_URL }]);
      const opened = await panel.settle();
      expect(state.videoControl.selectedTab).toBe('video-url');
      expect(opened.primary).toMatchObject({ label: 'Deactivate', enabled: true });
    });

    it('should allow deactivation after returning to URL without retyping', async () => {
      for (const otherSource of ['dcl-cast', 'live'] as const) {
        selectVideoSubTab(otherSource);
        await panel.settle();

        selectVideoSubTab('video-url');
        const reentry = await panel.frame();
        expect(reentry.input).toBe(RUNNING_URL);
        expect(reentry.primary).toMatchObject({ label: 'Deactivate', enabled: true });

        const settled = await panel.settle();
        expect(settled.input).toBe(RUNNING_URL);
        expect(settled.primary).toMatchObject({ label: 'Deactivate', enabled: true });
      }
      // Moving between sources changes nothing on the screen.
      expect(mocks.emitSetVideo).not.toHaveBeenCalled();

      const { primary } = await panel.settle();
      panel.press(primary);
      expect(mocks.emitSetVideo).toHaveBeenCalledTimes(1);
      expect(mocks.emitSetVideo).toHaveBeenCalledWith(panel.screenEntities[0], {
        src: '',
        playing: true,
      });
    });

    it('should keep offering Deactivate after several round trips', async () => {
      for (let i = 0; i < 3; i++) {
        selectVideoSubTab(i % 2 ? 'live' : 'dcl-cast');
        await panel.frame();
        selectVideoSubTab('video-url');
        const reentry = await panel.frame();
        expect(reentry.primary).toMatchObject({ label: 'Deactivate', enabled: true });
      }
    });

    it('should show the new source when it changed while the URL panel was closed', async () => {
      selectVideoSubTab('dcl-cast');
      await panel.frame();
      panel.setSource(0, OTHER_URL);
      await panel.frame();

      selectVideoSubTab('video-url');
      const reentry = await panel.frame();
      expect(reentry.input).toBe(OTHER_URL);
      expect(reentry.primary).toMatchObject({ label: 'Deactivate', enabled: true });
    });

    it('should offer Update for a different URL and submit that URL', async () => {
      panel.type(OTHER_URL);
      const { primary } = await panel.settle();
      expect(primary).toMatchObject({ label: 'Update', enabled: true });

      panel.press(primary);
      expect(mocks.emitSetVideo).toHaveBeenCalledTimes(1);
      expect(mocks.emitSetVideo).toHaveBeenCalledWith(panel.screenEntities[0], {
        src: OTHER_URL,
        playing: true,
      });
    });

    it('should offer Deactivate again when the typed URL goes back to the running one', async () => {
      panel.type(OTHER_URL);
      await panel.settle();
      panel.type(RUNNING_URL);
      const { primary } = await panel.settle();
      expect(primary).toMatchObject({ label: 'Deactivate', enabled: true });
    });

    it('should not allow updating to something that is not a video URL', async () => {
      panel.type('not a url');
      const { primary } = await panel.settle();
      expect(primary).toMatchObject({ label: 'Update', enabled: false });

      panel.press(primary);
      expect(mocks.emitSetVideo).not.toHaveBeenCalled();
    });

    it('should keep what the admin is typing across unrelated renders', async () => {
      panel.type(OTHER_URL);
      await panel.settle();
      // The screen keeps changing (volume, playback) while the admin types.
      for (let i = 0; i < 5; i++) {
        panel.setSource(0, RUNNING_URL);
        const { primary } = await panel.frame();
        expect(primary).toMatchObject({ label: 'Update', enabled: true });
      }

      const { primary } = await panel.settle();
      panel.press(primary);
      expect(mocks.emitSetVideo).toHaveBeenCalledWith(panel.screenEntities[0], {
        src: OTHER_URL,
        playing: true,
      });
    });

    it('should keep offering Deactivate when the renderer reports the text the field opened with', async () => {
      selectVideoSubTab('dcl-cast');
      await panel.settle();

      selectVideoSubTab('video-url');
      const reentry = await panel.frame();
      await panel.settle();
      // The renderer reports the field's text a few ticks after creating it. React does not write
      // a value back when it matches the last reported one, so a stale report would stick.
      panel.type(reentry.input ?? '');
      const { primary } = await panel.settle();
      expect(primary).toMatchObject({ label: 'Deactivate', enabled: true });
    });
  });

  describe('and the screen has no video', () => {
    beforeEach(async () => {
      panel = createPanel([{ src: '' }]);
      await panel.settle();
      selectVideoSubTab('video-url');
    });

    it('should start with an empty URL and a disabled Activate', async () => {
      const reentry = await panel.frame();
      expect(reentry.input).toBe('');
      expect(reentry.primary).toMatchObject({ label: 'Activate', enabled: false });
    });

    it('should activate a typed video URL', async () => {
      await panel.settle();
      panel.type(OTHER_URL);
      const { primary } = await panel.settle();
      expect(primary).toMatchObject({ label: 'Activate', enabled: true });

      panel.press(primary);
      expect(mocks.emitSetVideo).toHaveBeenCalledWith(panel.screenEntities[0], {
        src: OTHER_URL,
        playing: true,
      });
    });
  });

  describe('and the screen is showing a live stream', () => {
    beforeEach(async () => {
      panel = createPanel([{ src: 'livekit-video://current-stream' }]);
      await panel.settle();
      selectVideoSubTab('video-url');
    });

    it('should not put the stream address in the URL field and should offer Activate', async () => {
      const reentry = await panel.frame();
      expect(reentry.input).toBe('');
      expect(reentry.primary).toMatchObject({ label: 'Activate', enabled: false });
    });
  });

  describe('and the scene has more than one screen', () => {
    beforeEach(async () => {
      panel = createPanel([{ src: RUNNING_URL }, { src: OTHER_URL }]);
      await panel.settle();
    });

    it("should load the selected screen's URL and deactivate only that screen", async () => {
      selectVideoPlayer(1);
      await panel.frame();
      const reentry = await panel.frame();
      expect(state.videoControl.selectedTab).toBe('video-url');
      expect(reentry.input).toBe(OTHER_URL);
      expect(reentry.primary).toMatchObject({ label: 'Deactivate', enabled: true });

      panel.press(reentry.primary);
      expect(mocks.emitSetVideo).toHaveBeenCalledTimes(1);
      expect(mocks.emitSetVideo).toHaveBeenCalledWith(panel.screenEntities[1], {
        src: '',
        playing: true,
      });
    });
  });
});
