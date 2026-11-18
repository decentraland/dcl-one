import * as BABYLON from '@babylonjs/core';

import { getPointerCoords } from './mouse-utils';

describe('getPointerCoords', () => {
  let scene: BABYLON.Scene;

  beforeEach(() => {
    scene = new BABYLON.Scene(
      new BABYLON.NullEngine({ renderWidth: 400, renderHeight: 200 } as never),
    );
    const camera = new BABYLON.FreeCamera('camera', new BABYLON.Vector3(0, 10, 0), scene);
    camera.setTarget(BABYLON.Vector3.Zero());
    BABYLON.MeshBuilder.CreateGround('ground', { width: 100, height: 100 }, scene);
  });

  afterEach(() => {
    scene.getEngine().dispose();
  });

  // A drop ends an HTML5 drag, which sends Babylon no pointer events: the point has to come
  // back without waiting for the next one, or the item only appears once the mouse moves.
  it('answers right away, without a pointer event', () => {
    const point = getPointerCoords(scene, { x: 0, y: 0 });
    expect(point).not.toBeNull();
    expect(point!.x).toBeCloseTo(0);
    expect(point!.z).toBeCloseTo(0);
  });

  it('picks at the given viewport point, not the last pointer position', () => {
    const centre = getPointerCoords(scene, { x: 0, y: 0 })!;
    const right = getPointerCoords(scene, { x: 0.5, y: 0 })!;
    expect(right.x).not.toBeCloseTo(centre.x);
  });

  it('returns null when the ray misses everything', () => {
    scene.getMeshByName('ground')!.dispose();
    expect(getPointerCoords(scene, { x: 0, y: 0 })).toBeNull();
  });
});
