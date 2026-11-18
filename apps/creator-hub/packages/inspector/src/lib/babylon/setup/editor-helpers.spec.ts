import { Mesh, NullEngine, Scene, TransformNode } from '@babylonjs/core';
import type { EffectLayer, UtilityLayerRenderer } from '@babylonjs/core';

import { areEditorHelpersHidden, hideEditorHelpers } from './editor-helpers';

describe('when hiding the editor helpers for a screenshot', () => {
  let engine: NullEngine;
  let scene: Scene;
  let gizmoLayer: UtilityLayerRenderer;
  let selectionHighlight: EffectLayer;
  let otherEffect: EffectLayer;
  let parcelGrid: TransformNode;
  let spawnPoints: TransformNode;
  let groundGrid: Mesh;
  let particleHelper: Mesh;
  let outOfBoundsMesh: Mesh;
  let sceneMesh: Mesh;
  let restore: () => void;

  beforeEach(() => {
    engine = new NullEngine();
    scene = new Scene(engine);
    gizmoLayer = { shouldRender: true } as UtilityLayerRenderer;
    selectionHighlight = { name: 'selection_highlight', isEnabled: true } as EffectLayer;
    otherEffect = { name: 'glow', isEnabled: true } as EffectLayer;
    scene.effectLayers.push(selectionHighlight, otherEffect);
    parcelGrid = new TransformNode('layout', scene);
    spawnPoints = new TransformNode('spawn_points', scene);
    groundGrid = new Mesh('BackgroundPlane', scene);
    particleHelper = new Mesh('ps-helper-512', scene);
    outOfBoundsMesh = new Mesh('crate', scene);
    outOfBoundsMesh.showBoundingBox = true;
    sceneMesh = new Mesh('tree', scene);

    restore = hideEditorHelpers(scene, [gizmoLayer]);
  });

  afterEach(() => {
    engine.dispose();
  });

  it('should stop drawing the gizmos', () => {
    expect(gizmoLayer.shouldRender).toBe(false);
  });

  it('should hide the selection outline and keep the other effects', () => {
    expect(selectionHighlight.isEnabled).toBe(false);
    expect(otherEffect.isEnabled).toBe(true);
  });

  it('should hide the grids, the spawn points and the particle helpers', () => {
    expect(parcelGrid.isEnabled(false)).toBe(false);
    expect(spawnPoints.isEnabled(false)).toBe(false);
    expect(groundGrid.isEnabled(false)).toBe(false);
    expect(particleHelper.isEnabled(false)).toBe(false);
  });

  it('should hide the out-of-bounds boxes', () => {
    expect(outOfBoundsMesh.showBoundingBox).toBe(false);
  });

  it('should keep the scene content visible', () => {
    expect(sceneMesh.isEnabled(false)).toBe(true);
  });

  it('should report the helpers as hidden', () => {
    expect(areEditorHelpersHidden(scene)).toBe(true);
  });

  describe('and the screenshot is taken', () => {
    beforeEach(() => {
      restore();
    });

    it('should show everything it hid again', () => {
      expect(gizmoLayer.shouldRender).toBe(true);
      expect(selectionHighlight.isEnabled).toBe(true);
      expect(parcelGrid.isEnabled(false)).toBe(true);
      expect(groundGrid.isEnabled(false)).toBe(true);
      expect(particleHelper.isEnabled(false)).toBe(true);
      expect(outOfBoundsMesh.showBoundingBox).toBe(true);
      expect(areEditorHelpersHidden(scene)).toBe(false);
    });
  });
});

describe('when hiding the editor helpers while the user had some of them off', () => {
  let engine: NullEngine;
  let scene: Scene;
  let parcelGrid: TransformNode;
  let gizmoLayer: UtilityLayerRenderer;

  beforeEach(() => {
    engine = new NullEngine();
    scene = new Scene(engine);
    parcelGrid = new TransformNode('layout', scene);
    parcelGrid.setEnabled(false);
    gizmoLayer = { shouldRender: false } as UtilityLayerRenderer;

    hideEditorHelpers(scene, [gizmoLayer])();
  });

  afterEach(() => {
    engine.dispose();
  });

  it('should leave them off after the screenshot', () => {
    expect(parcelGrid.isEnabled(false)).toBe(false);
    expect(gizmoLayer.shouldRender).toBe(false);
  });
});
