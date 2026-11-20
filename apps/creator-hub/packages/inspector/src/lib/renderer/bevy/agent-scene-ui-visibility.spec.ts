import type {
  ConsoleCommandRunner,
  SceneUiVisibility,
} from '../../../../agents/bevy/src/scene-ui-visibility';
import {
  createSceneUiVisibility,
  sceneUiVisibleAfter,
  showUiArgs,
} from '../../../../agents/bevy/src/scene-ui-visibility';

type Call = [string, string[]];

describe('createSceneUiVisibility', () => {
  let calls: Call[];
  let run: ConsoleCommandRunner;
  let sceneUi: SceneUiVisibility;

  beforeEach(() => {
    calls = [];
    run = vi.fn(async (command: string, args: string[]) => {
      calls.push([command, args]);
      return `${args[0]}: ${args[1]}`;
    });
    sceneUi = createSceneUiVisibility({ getRunner: () => run });
  });

  describe('when the editor boots', () => {
    it('should want the scene UI hidden', () => {
      expect(sceneUi.isVisible()).toBe(false);
    });

    it('should hide it with the engine-wide default, needing no pinned scene hash', async () => {
      await sceneUi.reassert();
      expect(calls).toEqual([['show_ui', ['all', 'false']]]);
    });
  });

  describe('when the scene is played and then stopped', () => {
    beforeEach(async () => {
      await sceneUi.set(true);
      await sceneUi.set(false);
    });

    it('should show the UI on Play and hide it again on Stop', () => {
      expect(calls).toEqual([
        ['show_ui', ['all', 'true']],
        ['show_ui', ['all', 'false']],
      ]);
    });
  });

  describe('when Play arrives while an earlier hide is still in flight', () => {
    let releaseFirst: () => void;

    beforeEach(() => {
      let first = true;
      run = vi.fn((command: string, args: string[]) => {
        calls.push([command, args]);
        if (!first) return Promise.resolve('ok');
        first = false;
        return new Promise<string>(resolve => {
          releaseFirst = () => resolve('ok');
        });
      });
      sceneUi = createSceneUiVisibility({ getRunner: () => run });
    });

    it('should send nothing else until the in-flight hide settles, then show', async () => {
      const boot = sceneUi.reassert();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      const play = sceneUi.set(true);
      await Promise.resolve();
      expect(calls).toEqual([['show_ui', ['all', 'false']]]);
      releaseFirst();
      await Promise.all([boot, play]);
      expect(calls).toEqual([
        ['show_ui', ['all', 'false']],
        ['show_ui', ['all', 'true']],
      ]);
    });

    it('should end on the shown state when the boot reassert queues after Play', async () => {
      const firstHide = sceneUi.reassert();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      const play = sceneUi.set(true);
      const lateBoot = sceneUi.reassert();
      releaseFirst();
      await Promise.all([firstHide, play, lateBoot]);
      expect(calls).toEqual([
        ['show_ui', ['all', 'false']],
        ['show_ui', ['all', 'true']],
        ['show_ui', ['all', 'true']],
      ]);
    });
  });

  describe('when a command fails', () => {
    let onError: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      let first = true;
      onError = vi.fn();
      run = vi.fn(async (command: string, args: string[]) => {
        calls.push([command, args]);
        if (first) {
          first = false;
          throw new Error('console not ready');
        }
        return 'ok';
      });
      sceneUi = createSceneUiVisibility({ getRunner: () => run, onError });
    });

    it('should report it and still send the next command', async () => {
      await sceneUi.reassert();
      await sceneUi.reassert();
      expect(onError).toHaveBeenCalledTimes(1);
      expect(calls).toEqual([
        ['show_ui', ['all', 'false']],
        ['show_ui', ['all', 'false']],
      ]);
    });
  });

  describe('when the engine API is unavailable', () => {
    beforeEach(() => {
      sceneUi = createSceneUiVisibility({ getRunner: () => null });
    });

    it('should keep the wanted state for the next send', async () => {
      await sceneUi.set(true);
      expect(sceneUi.isVisible()).toBe(true);
    });
  });
});

describe('sceneUiVisibleAfter', () => {
  describe('when the scene is paused after playing', () => {
    it('should keep the UI shown', () => {
      expect(sceneUiVisibleAfter(sceneUiVisibleAfter(false, 'play'), 'pause')).toBe(true);
    });
  });

  describe('when a stopped scene is frozen again', () => {
    it('should keep the UI hidden', () => {
      expect(sceneUiVisibleAfter(false, 'pause')).toBe(false);
    });
  });

  describe('when a paused scene is stopped', () => {
    it('should hide the UI', () => {
      expect(sceneUiVisibleAfter(true, 'stop')).toBe(false);
    });
  });
});

describe('showUiArgs', () => {
  it('should target every non-system scene UI', () => {
    expect(showUiArgs(false)).toEqual(['all', 'false']);
    expect(showUiArgs(true)).toEqual(['all', 'true']);
  });
});
