import { describe, expect, it } from 'vitest';

import { inspectRiggedModel, isWearableCandidate, suggestCategory } from './wearable-models';

function glb(json: object): Uint8Array {
  const text = new TextEncoder().encode(JSON.stringify(json));
  const padded = new Uint8Array(Math.ceil(text.length / 4) * 4).fill(0x20);
  padded.set(text);
  const bytes = new Uint8Array(12 + 8 + padded.length);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, 0x46546c67, true);
  view.setUint32(4, 2, true);
  view.setUint32(8, bytes.length, true);
  view.setUint32(12, padded.length, true);
  view.setUint32(16, 0x4e4f534a, true);
  bytes.set(padded, 20);
  return bytes;
}

const rig = { nodes: [{ name: 'Avatar_Hips' }, { name: 'Armature' }], skins: [{ joints: [0] }] };

describe('inspectRiggedModel', () => {
  describe('when the model is skinned to the avatar armature with no animation', () => {
    it('should read it as a wearable and suggest a slot from its mesh names', () => {
      expect(inspectRiggedModel(glb({ ...rig, meshes: [{ name: 'Jetpack_uBody' }] }))).toEqual({
        suggestedCategory: 'upper_body',
      });
    });
  });

  describe('when it has an animation', () => {
    it('should not read it as a wearable, since it is an emote', () => {
      expect(inspectRiggedModel(glb({ ...rig, animations: [{}] }))).toBeUndefined();
    });
  });

  describe('when it has no avatar bones', () => {
    it('should not read it as a wearable', () => {
      expect(
        inspectRiggedModel(glb({ nodes: [{ name: 'Door' }], skins: [{ joints: [0] }] })),
      ).toBeUndefined();
    });
  });
});

describe('suggestCategory', () => {
  it('should read the slot wearable meshes are usually named after', () => {
    expect(suggestCategory(['M_Feet_Sneakers'])).toBe('feet');
    expect(suggestCategory(['Beard_FacialHair'])).toBe('facial_hair');
    expect(suggestCategory(['Cube'])).toBeUndefined();
  });
});

describe('isWearableCandidate', () => {
  it('should read only GLBs whose name marks nothing, outside asset packs', () => {
    expect(isWearableCandidate('assets/Models/jetpack_suit.glb')).toBe(true);
    expect(isWearableCandidate('assets/Models/cap_hat.glb')).toBe(false);
    expect(isWearableCandidate('assets/Models/dance_emote.glb')).toBe(false);
    expect(isWearableCandidate('assets/asset-packs/chair/chair.glb')).toBe(false);
  });
});
