/**
 * The inspected scene's own UI (its react-ecs window UI) in the editor viewport:
 * hidden while the scene is stopped (it loaded or was stopped, and has not played
 * since), shown from Play on. A Pause keeps it as it was, so a paused scene still
 * shows its UI.
 *
 * Enacted with the engine's `/show_ui all <true|false>`. The `all` form is what
 * makes it stick: it sets the engine's default for every non-system scene UI, and
 * the engine reads that default whenever it creates a scene's UI root, so a root
 * created after the command (the scene renders its UI late, the player settles
 * into the scene after boot, a `reload` brings up a fresh instance) comes up in
 * the wanted state. The per-scene form (`show_ui <hash> false`) is only stored
 * when the scene already has a UI root, so sent before that root exists it is
 * acknowledged and then lost. The system scene, which carries this agent's gizmo
 * overlay, is always exempt from `show_ui`.
 *
 * Pure and SDK-free so the inspector's unit tests can import it
 * (`src/lib/renderer/bevy/agent-scene-ui-visibility.spec.ts`).
 */

/** Runs an engine console command (the agent's `BevyApi.consoleCommand`). */
export type ConsoleCommandRunner = (command: string, args: string[]) => Promise<string>;

export interface SceneUiVisibility {
  /** Record the wanted visibility and send it to the engine. */
  set(visible: boolean): Promise<void>;
  /** Send the wanted visibility again (after an engine boot or a scene reload). */
  reassert(): Promise<void>;
  /** The wanted visibility. Starts hidden: the editor opens the scene stopped. */
  isVisible(): boolean;
}

export interface SceneUiVisibilityOptions {
  /** The console runner, or null while the engine API is unavailable. */
  getRunner: () => ConsoleCommandRunner | null;
  onError?: (error: unknown) => void;
}

/** What changes the scene UI: Play, Pause (freeze where it is) or Stop (reset to
 * the initial state). */
export type SceneRunEvent = 'play' | 'pause' | 'stop';

export function sceneUiVisibleAfter(current: boolean, event: SceneRunEvent): boolean {
  if (event === 'play') return true;
  if (event === 'stop') return false;
  return current;
}

export function showUiArgs(visible: boolean): string[] {
  return ['all', visible ? 'true' : 'false'];
}

/**
 * Sends are serialized, and each one sends the visibility wanted when it runs,
 * not when it was queued. So a stale hide queued behind a slow boot can never
 * land after a newer show (Play pressed while the agent is still booting): the
 * last command the engine receives is always the latest wanted state.
 */
export function createSceneUiVisibility(options: SceneUiVisibilityOptions): SceneUiVisibility {
  let visible = false;
  let queue: Promise<void> = Promise.resolve();

  const send = (): Promise<void> => {
    queue = queue.then(async () => {
      const run = options.getRunner();
      if (!run) return;
      try {
        await run('show_ui', showUiArgs(visible));
      } catch (error) {
        options.onError?.(error);
      }
    });
    return queue;
  };

  return {
    set: next => {
      visible = next;
      return send();
    },
    reassert: send,
    isVisible: () => visible,
  };
}
