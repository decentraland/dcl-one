import * as BABYLON from '@babylonjs/core';
import type { Entity } from '@dcl/ecs';

import { EcsEntity } from '../../babylon/decentraland/EcsEntity';
import type { SceneContext } from '../../babylon/decentraland/SceneContext';
import { getLayoutManager } from '../../babylon/decentraland/layout-manager';
import { computeEntitiesOutsideLayout } from './metrics';

describe('computeEntitiesOutsideLayout', () => {
  let scene: BABYLON.Scene;
  let entities: EcsEntity[];
  let context: SceneContext;

  beforeEach(() => {
    scene = new BABYLON.Scene(new BABYLON.NullEngine());
    // One parcel: x and z from 0 to 16.
    getLayoutManager(scene).setLayout({ base: { x: 0, y: 0 }, parcels: [{ x: 0, y: 0 }] });
    // The layout has been on screen for many frames by the time anything is dropped.
    for (const mesh of scene.meshes) mesh.computeWorldMatrix(true);
    entities = [];
    context = { getAllEntities: () => entities } as unknown as SceneContext;
  });

  afterEach(() => {
    scene.getEngine().dispose();
  });

  // A model that has just loaded, before any frame renders: its bounding mesh is built and the
  // list recomputed in the same tick, so no world matrix has been computed yet.
  function dropModelAt(id: number, position: BABYLON.Vector3) {
    const ctx = { deref: () => undefined } as unknown as WeakRef<SceneContext>;
    const entity = new EcsEntity(id as Entity, ctx, scene);
    entity.position = position;
    const box = BABYLON.MeshBuilder.CreateBox(`box-${id}`, { size: 1 }, scene);
    box.material = new BABYLON.StandardMaterial(`mat-${id}`, scene);
    box.parent = entity;
    entity.generateBoundingBox();
    entities.push(entity);
    return entity;
  }

  it('does not report a just-dropped model inside the parcel', () => {
    dropModelAt(512, new BABYLON.Vector3(8, 0, 8));
    expect(computeEntitiesOutsideLayout(scene, context)).toEqual([]);
  });

  it('reports a just-dropped model outside the parcel', () => {
    dropModelAt(513, new BABYLON.Vector3(-8, 0, 8));
    expect(computeEntitiesOutsideLayout(scene, context)).toEqual([513]);
  });
});
