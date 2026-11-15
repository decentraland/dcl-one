import { describe, expect, it } from 'vitest';

import { mergeDeep } from './deep-merge';

describe('mergeDeep', () => {
  describe('when both objects have a plain object under the same key', () => {
    it('should merge them key by key, at each depth', () => {
      const base = { display: { title: 'Scene', navmapThumbnail: 'a.png' }, main: 'bin/index.js' };
      const changes = { display: { title: 'New title' } };

      expect(mergeDeep(base, changes)).toEqual({
        display: { title: 'New title', navmapThumbnail: 'a.png' },
        main: 'bin/index.js',
      });
    });
  });

  describe('when both objects have an array under the same key', () => {
    it('should take the array of the changes and not join them', () => {
      const merged = mergeDeep(
        { scene: { parcels: ['0,0', '0,1'], base: '0,0' } },
        { scene: { parcels: ['5,5'] } },
      );

      expect(merged).toEqual({ scene: { parcels: ['5,5'], base: '0,0' } });
    });
  });

  describe('when the changes have a value of another type than the base', () => {
    it('should take the value of the changes', () => {
      const merged = mergeDeep(
        { tags: ['a'], display: { title: 'Scene' }, owner: 'me' },
        { tags: { first: 'a' }, display: 'none', owner: null },
      );

      expect(merged).toEqual({ tags: { first: 'a' }, display: 'none', owner: null });
    });
  });

  describe('when the changes have a key with the value undefined', () => {
    it('should set the key to undefined', () => {
      const merged = mergeDeep({ worldConfiguration: { name: 'world.dcl.eth' } }, {
        worldConfiguration: undefined,
      } as object);

      expect(Object.keys(merged)).toEqual(['worldConfiguration']);
      expect(merged.worldConfiguration).toBeUndefined();
    });
  });

  describe('when the result is changed', () => {
    it('should not change the plain objects of the two sources', () => {
      const base = { display: { title: 'Scene' } };
      const changes = { spawnPoints: { default: { name: 'spawn' } } };

      const merged = mergeDeep(base, changes) as typeof base & typeof changes;
      merged.display.title = 'Changed';
      merged.spawnPoints.default.name = 'changed';

      expect(base.display.title).toBe('Scene');
      expect(changes.spawnPoints.default.name).toBe('spawn');
    });
  });

  describe('when a source has a key that can change how an object behaves', () => {
    it('should leave the key out', () => {
      const changes = JSON.parse(
        '{"__proto__":{"polluted":true},"constructor":1,"display":{"toString":2,"title":"Scene"}}',
      );

      const merged = mergeDeep({}, changes) as { polluted?: boolean; display: object };

      expect(merged.polluted).toBeUndefined();
      expect(Object.keys(merged)).toEqual(['display']);
      expect(Object.keys(merged.display)).toEqual(['title']);
      expect(({} as { polluted?: boolean }).polluted).toBeUndefined();
    });
  });
});
