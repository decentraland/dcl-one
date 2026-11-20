import { describe, expect, it } from 'vitest';

import {
  BODY_SHAPE_A,
  BODY_SHAPE_B,
  getBaseOutfitUrns,
  getSceneWearableCategory,
  getSceneWearableName,
  isSceneWearablePath,
  swapBaseOutfit,
} from './wearables';

describe('scene wearable paths', () => {
  describe('when the file is named name_category.glb', () => {
    it('should take the category from the name', () => {
      expect(getSceneWearableCategory('assets/scene/cap_hat.glb')).toBe('hat');
      expect(getSceneWearableName('assets/scene/cap_hat.glb')).toBe('cap');
    });
  });

  describe('when the category has an underscore', () => {
    it('should keep the whole category', () => {
      expect(getSceneWearableCategory('denim_jacket_upper_body.glb')).toBe('upper_body');
      expect(getSceneWearableName('denim_jacket_upper_body.glb')).toBe('denim_jacket');
    });
  });

  describe('when one category ends another', () => {
    it('should take the longer one', () => {
      expect(getSceneWearableCategory('beard_facial_hair.glb')).toBe('facial_hair');
      expect(getSceneWearableCategory('crown_top_head.glb')).toBe('top_head');
    });
  });

  describe('when the extension is upper case', () => {
    it('should still read the category', () => {
      expect(getSceneWearableCategory('Robot_SKIN.GLB')).toBe('skin');
    });
  });

  describe('when the file is not a wearable', () => {
    it.each([
      'chair.glb',
      'hat.glb',
      '_hat.glb',
      'cap_hat.gltf',
      'cap_hat.png',
      'cap_body_shape.glb',
    ])('should not read %s as one', path => {
      expect(isSceneWearablePath(path)).toBe(false);
    });
  });
});

describe('swapBaseOutfit', () => {
  describe('when the body shape changes', () => {
    it("should swap the old body's base items and keep the rest", () => {
      const hat = 'urn:decentraland:matic:collections-v2:0xabc:2';
      const outfit = [...getBaseOutfitUrns(BODY_SHAPE_B), hat];
      expect(swapBaseOutfit(outfit, BODY_SHAPE_A)).toEqual([
        ...getBaseOutfitUrns(BODY_SHAPE_A),
        hat,
      ]);
    });
  });
});
