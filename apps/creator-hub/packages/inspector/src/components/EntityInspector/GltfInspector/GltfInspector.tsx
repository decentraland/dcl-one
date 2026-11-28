import { useCallback } from 'react';

import { withSdk } from '../../../hoc/withSdk';
import { useHasComponent } from '../../../hooks/sdk/useHasComponent';
import { useComponentInput } from '../../../hooks/sdk/useComponentInput';
import { useAssetOptions } from '../../../hooks/useAssetOptions';
import { Block } from '../../Block';
import { Container } from '../../Container';
import { FileUploadField, Dropdown, Label, InfoTooltip } from '../../ui';
import { Message, MessageType } from '../../ui/Message';
import { ACCEPTED_FILE_TYPES } from '../../ui/FileUploadField/types';
import { useAppSelector } from '../../../redux/hooks';
import { selectAssetCatalog } from '../../../redux/app';
import {
  fromGltf,
  toGltf,
  isValidInput,
  COLLISION_LAYERS,
  isModel,
  VISIBLE_COLLIDER_TRIANGLE_LIMIT,
} from './utils';
import { useVisibleTriangles } from './useVisibleTriangles';
import type { Props } from './types';

import './GltfInspector.css';

export default withSdk<Props>(({ sdk, entity, initialOpen = true }) => {
  const files = useAppSelector(selectAssetCatalog);
  const modelOptions = useAssetOptions(ACCEPTED_FILE_TYPES['model']);
  const { GltfContainer } = sdk.components;

  const hasGltf = useHasComponent(entity, GltfContainer);
  const handleInputValidation = useCallback(
    ({ src }: { src: string }) => !!files && isValidInput(files, src),
    [files],
  );
  const { getInputProps, isValid } = useComponentInput(entity, GltfContainer, fromGltf, toGltf, {
    validateInput: handleInputValidation,
    deps: [files],
  });

  const handleRemove = useCallback(async () => {
    const { VisibilityComponent, MeshCollider } = sdk.components;
    const hasMeshCollider = MeshCollider.has(entity);
    const hasVisibility = VisibilityComponent.has(entity);

    sdk.operations.removeComponent(entity, GltfContainer);

    if (hasVisibility && !hasMeshCollider) {
      sdk.operations.removeComponent(entity, VisibilityComponent);
    }

    await sdk.operations.dispatch();
  }, [sdk, entity]);

  const handleDrop = useCallback(async (src: string) => {
    const { operations } = sdk;
    operations.updateValue(GltfContainer, entity, { src });
    await operations.dispatch();
  }, []);

  const srcProps = getInputProps('src');
  const visibleLayerProps = getInputProps('visibleMeshesCollisionMask');
  const visibleCollides = Number(visibleLayerProps.value) > 0;
  const triangles = useVisibleTriangles(
    typeof srcProps.value === 'string' ? srcProps.value : undefined,
    hasGltf && visibleCollides,
  );
  const tooManyTriangles =
    visibleCollides && triangles !== null && triangles > VISIBLE_COLLIDER_TRIANGLE_LIMIT;

  if (!hasGltf) return null;

  return (
    <Container
      label="GLTF"
      className="GltfInspector"
      initialOpen={initialOpen}
      rightContent={
        <InfoTooltip
          text="The GLTF assigns a 3D model file for the item's visible shape. It also handles collisions, to make an item clickable or block the player from walking through it."
          type="help"
        />
      }
      component={GltfContainer}
      entity={entity}
      onRemoveContainer={handleRemove}
    >
      <Block>
        <FileUploadField
          {...srcProps}
          label="Path"
          accept={ACCEPTED_FILE_TYPES['model']}
          options={modelOptions}
          onDrop={handleDrop}
          error={files && !isValid}
          isValidFile={isModel}
        />
      </Block>
      <div className="column">
        <Label
          text="Collisions"
          header
        />
        <Block>
          <Dropdown
            label={
              <>
                Visible layer{' '}
                <InfoTooltip
                  text={
                    <>
                      Collisions for the meshes you can see. <b>Important:</b> any layer other than
                      None duplicates the model's whole visible geometry to build the collider,
                      which is expensive for performance. Avoid it on models with many triangles:
                      keep this on None and give the model a simpler collider instead.
                    </>
                  }
                  type="help"
                />
              </>
            }
            aria-label="Visible layer"
            options={COLLISION_LAYERS}
            {...visibleLayerProps}
          />
          <Dropdown
            label={
              <>
                Invisible layer{' '}
                <InfoTooltip
                  text="Collisions for the model's invisible collider meshes, the ones with “collider” in their name. These are simple shapes made for collisions, so this is the cheap and recommended way to make a model solid or clickable."
                  type="help"
                />
              </>
            }
            aria-label="Invisible layer"
            options={COLLISION_LAYERS}
            {...getInputProps('invisibleMeshesCollisionMask')}
          />
        </Block>
        {tooManyTriangles && (
          <Message
            className="GltfColliderWarning"
            type={MessageType.WARNING}
            text={`This model has ${triangles.toLocaleString()} triangles, all duplicated as a collider by the Visible layer. Set it to None and use a simpler collider mesh or a nested invisible wall entity.`}
          />
        )}
      </div>
    </Container>
  );
});
