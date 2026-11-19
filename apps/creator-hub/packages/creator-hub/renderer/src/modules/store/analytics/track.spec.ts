import { describe, expect, it } from 'vitest';

import { actions as editorActions } from '../editor';
import { analyticsConfig } from './track';

const publishScene = analyticsConfig[editorActions.publishScene.fulfilled.type];
const getState = (() => ({ editor: { project: { id: 'project-1' } } })) as never;
const payloadFor = (arg: { target?: string; targetContent?: string }) =>
  publishScene.getPayload({ type: '', payload: undefined, meta: { arg } } as never, getState);

describe('Publish Scene analytics', () => {
  describe('when the scene is published to a World', () => {
    it('should report the target as worlds', () => {
      expect(payloadFor({ targetContent: 'https://worlds.example.org' })).toEqual({
        project_id: 'project-1',
        target: 'worlds',
        targetContent: 'https://worlds.example.org',
      });
    });
  });

  describe('when the scene is published to LAND', () => {
    it('should report the catalyst it was sent to', () => {
      expect(payloadFor({ target: 'https://peer.example.org' })).toMatchObject({
        target: 'https://peer.example.org',
        targetContent: 'unknown',
      });
    });
  });
});
