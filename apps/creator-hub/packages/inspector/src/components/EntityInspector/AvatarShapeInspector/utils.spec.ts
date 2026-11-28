import { describe, expect, it } from 'vitest';

import { getSceneWearableCategory } from '../../../lib/logic/wearables';
import type { WearableItem } from './catalog';
import { BODY_SHAPE_B } from '../../../lib/logic/wearables';
import {
  addWearable,
  getNametagSuffix,
  getPreviewUrns,
  getSlotFileName,
  getWornBySlot,
  nextExpressionTimestamp,
  removeWearableAt,
  toPreviewColor,
  toWearableRows,
} from './utils';

const HELMET = 'urn:decentraland:matic:collections-v2:0xabc:1';
const HAT = 'urn:decentraland:matic:collections-v2:0xabc:2';
const TROUSERS = 'urn:decentraland:off-chain:base-avatars:f_brown_trousers';

const resolved = new Map<string, WearableItem>([
  [HELMET, { id: HELMET, name: 'Space Helmet', category: 'helmet', source: 'catalog' }],
  [HAT, { id: HAT, name: 'Wizard Hat', category: 'hat', source: 'catalog' }],
  [TROUSERS, { id: TROUSERS, name: 'Brown Trousers', category: 'lower_body', source: 'base' }],
]);

const categoryOf = (value: string) =>
  resolved.get(value)?.category ?? getSceneWearableCategory(value);

describe('toWearableRows', () => {
  describe('when the list mixes catalog URNs and scene files', () => {
    it('should describe each item with its name, slot and source', () => {
      const rows = toWearableRows([HELMET, 'assets/wearables/apron_upper_body.glb'], resolved);
      expect(
        rows.map(({ name, categoryLabel, source }) => ({ name, categoryLabel, source })),
      ).toEqual([
        { name: 'Space Helmet', categoryLabel: 'Helmet', source: 'catalog' },
        { name: 'apron', categoryLabel: 'Upper body', source: 'scene' },
      ]);
    });
  });

  describe('when two items take the same slot', () => {
    it('should mark the earlier one as replaced by the later one', () => {
      const rows = toWearableRows(['old_hat.glb', HAT], resolved);
      expect(rows[0].replacedBy).toBe('Wizard Hat');
      expect(rows[1].replacedBy).toBeUndefined();
      expect(getWornBySlot(rows).get('hat')?.name).toBe('Wizard Hat');
    });
  });

  describe('when a URN is not known to any catalog', () => {
    it('should show the last part of the URN', () => {
      const [row] = toWearableRows(['urn:decentraland:matic:collections-v2:0xdef:7'], new Map());
      expect(row.name).toBe('7');
      expect(row.invalid).toBe(false);
    });
  });

  describe('when an item is neither a URN nor a wearable file', () => {
    it('should flag it as invalid', () => {
      const [row] = toWearableRows(['models/chair.glb'], resolved);
      expect(row.invalid).toBe(true);
    });
  });
});

describe('addWearable', () => {
  describe('when the avatar already wears something in that slot', () => {
    it('should replace it and put the new item last', () => {
      expect(
        addWearable([HAT, TROUSERS], { id: 'cap_hat.glb', category: 'hat' }, categoryOf),
      ).toEqual([TROUSERS, 'cap_hat.glb']);
    });
  });

  describe('when the item is already in the list', () => {
    it('should not add it twice', () => {
      expect(addWearable([HAT, TROUSERS], { id: HAT, category: 'hat' }, categoryOf)).toEqual([
        TROUSERS,
        HAT,
      ]);
    });
  });
});

describe('removeWearableAt', () => {
  it('should remove only the item at that position', () => {
    expect(removeWearableAt([HAT, HAT, TROUSERS], 1)).toEqual([HAT, TROUSERS]);
  });
});

describe('toPreviewColor', () => {
  it('should give the hex without the #, falling back to the default', () => {
    expect(toPreviewColor(undefined, { r: 1, g: 0, b: 0 })).toBe('FF0000');
    expect(toPreviewColor({ r: 0, g: 0, b: 1 }, { r: 1, g: 0, b: 0 })).toBe('0000FF');
  });
});

describe('getPreviewUrns', () => {
  describe('when the avatar wears nothing', () => {
    it('should still show the face an explorer adds', () => {
      expect(getPreviewUrns([], BODY_SHAPE_B)).toEqual([
        'urn:decentraland:off-chain:base-avatars:f_eyes_00',
        'urn:decentraland:off-chain:base-avatars:f_eyebrows_00',
        'urn:decentraland:off-chain:base-avatars:f_mouth_00',
      ]);
    });
  });

  describe('when the list holds scene files and replaced items', () => {
    it('should keep only the URNs the avatar wears', () => {
      const rows = toWearableRows(['old_hat.glb', HAT, 'apron_upper_body.glb'], resolved);
      expect(getPreviewUrns(rows, BODY_SHAPE_B)).toContain(HAT);
      expect(getPreviewUrns(rows, BODY_SHAPE_B)).not.toContain('apron_upper_body.glb');
    });
  });
});

describe('getSlotFileName', () => {
  it('should add the slot to the file name', () => {
    expect(getSlotFileName('assets/Models/jetpack_suit.glb', 'upper_body')).toBe(
      'jetpack_suit_upper_body.glb',
    );
  });
});

describe('getNametagSuffix', () => {
  it('should show the last four characters of the id, or #0000', () => {
    expect(getNametagSuffix('0x1234abcd')).toBe('#abcd');
    expect(getNametagSuffix('')).toBe('#0000');
  });
});

describe('nextExpressionTimestamp', () => {
  it('should count plays from 0, as a lamport counter', () => {
    expect(nextExpressionTimestamp(undefined)).toBe(0);
    expect(nextExpressionTimestamp(4)).toBe(5);
  });
});

describe('base outfit items', () => {
  it('should read their slot before the catalog answers', () => {
    const [row] = toWearableRows(['urn:decentraland:off-chain:base-avatars:bun_shoes'], new Map());
    expect(row.category).toBe('feet');
    expect(row.source).toBe('base');
  });
});
