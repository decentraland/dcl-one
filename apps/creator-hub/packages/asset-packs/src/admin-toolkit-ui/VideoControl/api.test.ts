import { beforeEach, describe, expect, it, vi } from 'vitest';

const comms = vi.hoisted(() => ({ queue: [] as { sender: string; data: string }[][] }));

// The scene runtime's comms: each drain returns the next batch.
vi.mock('~system/CommsApi', () => ({
  getActiveVideoStreams: async () => ({ streams: [] }),
  subscribeToTopic: async () => ({}),
  publishData: async () => ({}),
  consumeMessages: async () => comms.queue.shift() ?? [],
}));

import { consumePresentationMessages, resetPresentationTracking } from './api';

const state = (id: string) => ({
  sender: 'presentation-bot',
  data: JSON.stringify({
    type: 'presentation:state',
    id,
    fileName: id,
    currentSlide: 0,
    slideCount: 3,
    fileType: 'pdf',
  }),
});
const stopped = (id: string) => ({
  sender: 'presentation-bot',
  data: JSON.stringify({ type: 'presentation:stopped', id }),
});

describe('consumePresentationMessages', () => {
  beforeEach(() => {
    comms.queue = [];
    resetPresentationTracking();
  });

  it('ignores the stop of a replaced presentation in the same drain', async () => {
    comms.queue.push([state('old'), state('new'), stopped('old')]);
    expect(await consumePresentationMessages()).toMatchObject({ id: 'new' });
  });

  it('ignores the stop of a replaced presentation in a later drain', async () => {
    comms.queue.push([state('new')], [stopped('old')]);
    expect(await consumePresentationMessages()).toMatchObject({ id: 'new' });
    expect(await consumePresentationMessages()).toBeUndefined();
  });

  it('ends on the stop of the current presentation', async () => {
    comms.queue.push([state('new')], [stopped('new')]);
    expect(await consumePresentationMessages()).toMatchObject({ id: 'new' });
    expect(await consumePresentationMessages()).toBe('stopped');
  });

  it('ends on a stop before any state', async () => {
    comms.queue.push([stopped('any')]);
    expect(await consumePresentationMessages()).toBe('stopped');
  });
});
