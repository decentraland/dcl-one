import type { Entity, IEngine } from '@dcl/ecs';
import { CrdtMessageType } from '@dcl/ecs';

import type { FileSystemInterface } from '../types';
import type { InspectorPreferences } from '../../logic/preferences/types';
import type { StateProvider } from './state-manager';
import { StateManager } from './state-manager';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(r => (resolve = r));
  return { promise, resolve };
}

function createStateManager() {
  return new StateManager({
    fs: {} as FileSystemInterface,
    engine: {} as IEngine,
    getInspectorPreferences: () => ({}) as InspectorPreferences,
    compositePath: 'main.composite',
  });
}

// A provider whose scene write stays in flight until the test lets it finish.
function createSlowProvider(log: string[]) {
  const write = deferred();
  const provider: StateProvider = {
    name: 'slow',
    canHandle: () => true,
    async processOperation() {
      log.push('write:start');
      await write.promise;
      log.push('write:end');
    },
    async onTransactionComplete() {},
  };
  return { provider, finishWrite: write.resolve };
}

const transform = { componentName: 'core::Transform' } as never;

describe('StateManager.executeTransaction', () => {
  it('waits for an in-flight scene write instead of failing', async () => {
    const log: string[] = [];
    const stateManager = createStateManager();
    const { provider, finishWrite } = createSlowProvider(log);
    stateManager.registerProvider(provider);

    // The first drop's entity lands in the engine; its write starts on the next tick.
    stateManager.createOnChangeHandler()(
      512 as Entity,
      CrdtMessageType.PUT_COMPONENT,
      transform,
      {},
    );
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(log).toEqual(['write:start']);

    // The second drop imports its files while that write is still running.
    const imported = stateManager.executeTransaction('external', async () => {
      log.push('import');
      return 'done';
    });

    await new Promise(resolve => setTimeout(resolve, 0));
    expect(log).toEqual(['write:start']);

    finishWrite();
    await expect(imported).resolves.toBe('done');
    expect(log).toEqual(['write:start', 'write:end', 'import']);
  });

  it('runs overlapping explicit transactions one after the other', async () => {
    const log: string[] = [];
    const stateManager = createStateManager();
    const firstGate = deferred();

    const first = stateManager.executeTransaction('external', async () => {
      log.push('first:start');
      await firstGate.promise;
      log.push('first:end');
    });
    const second = stateManager.executeTransaction('external', async () => {
      log.push('second');
    });

    await new Promise(resolve => setTimeout(resolve, 0));
    expect(log).toEqual(['first:start']);

    firstGate.resolve();
    await Promise.all([first, second]);
    expect(log).toEqual(['first:start', 'first:end', 'second']);
  });

  it('keeps going after a transaction fails', async () => {
    const stateManager = createStateManager();
    const failed = stateManager.executeTransaction('external', async () => {
      throw new Error('boom');
    });
    const next = stateManager.executeTransaction('external', async () => 'ok');

    await expect(failed).rejects.toThrow('boom');
    await expect(next).resolves.toBe('ok');
  });
});
