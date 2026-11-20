import { Color3, Mesh, MeshBuilder, StandardMaterial } from '@babylonjs/core';
import type { PBAvatarShape } from '@dcl/ecs';
import { ComponentType } from '@dcl/ecs';

import type { ComponentOperation } from '../component-operations';
import type { EcsEntity } from '../EcsEntity';
import { toggleMeshSelection } from '../editorComponents/selection';

const DEFAULT_SKIN = new Color3(0.6, 0.462, 0.356);
const AVATAR_HEIGHT = 1.8;
const HEAD_DIAMETER = 0.36;

/**
 * The editor viewport cannot draw an avatar, so an entity with AvatarShape shows a mannequin of
 * an avatar's height in its skin colour: something to see, select and move. Explorers draw the
 * real avatar. Like a Placeholder, it is left out of the scene metrics.
 */
export const putAvatarShapeComponent: ComponentOperation = (entity, component) => {
  if (component.componentType !== ComponentType.LastWriteWinElementSet) return;
  const value = component.getOrNull(entity.entityId) as PBAvatarShape | null;
  removeMannequin(entity);
  if (!value) return;
  // On the Player, AvatarShape restyles each visitor's own avatar: there is no avatar to draw.
  if (entity.entityId === entity.context.deref()?.engine.PlayerEntity) return;

  const scene = entity.getScene();
  const bodyHeight = AVATAR_HEIGHT - HEAD_DIAMETER;
  const body = MeshBuilder.CreateCapsule(
    'avatar_shape_body',
    { height: bodyHeight, radius: 0.24, tessellation: 12 },
    scene,
  );
  body.position.y = bodyHeight / 2;
  const head = MeshBuilder.CreateSphere(
    'avatar_shape_head',
    { diameter: HEAD_DIAMETER, segments: 10 },
    scene,
  );
  head.position.y = AVATAR_HEIGHT - HEAD_DIAMETER / 2;
  const mannequin = Mesh.MergeMeshes([body, head], true);
  if (!mannequin) return;

  const material = new StandardMaterial(`avatar_shape_material_${entity.entityId}`, scene);
  const skin = value.skinColor;
  material.diffuseColor = skin ? new Color3(skin.r, skin.g, skin.b) : DEFAULT_SKIN;
  material.specularColor = Color3.Black();
  mannequin.material = material;
  mannequin.name = `avatar_shape_${entity.entityId}`;
  mannequin.metadata = { ...mannequin.metadata, isPlaceholder: true };
  mannequin.isPickable = true;
  mannequin.parent = entity;
  entity.avatarShape = mannequin;
  entity.generateBoundingBox();

  const context = entity.context.deref();
  if (context?.editorComponents.Hide.getOrNull(entity.entityId)?.value) {
    mannequin.setEnabled(false);
  }

  toggleMeshSelection(mannequin, context?.editorComponents.Selection.has(entity.entityId) || false);
};

function removeMannequin(entity: EcsEntity) {
  if (!entity.avatarShape) return;
  entity.avatarShape.setEnabled(false);
  entity.avatarShape.parent = null;
  entity.avatarShape.dispose(false, true);
  delete entity.avatarShape;
}
