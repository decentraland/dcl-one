import { useMemo, useState } from 'react';
import type { WearableCategory as SchemaWearableCategory } from '@dcl/schemas';

import type { WearableCategory } from '../../lib/logic/wearables';
import { BODY_SHAPE_A, BODY_SHAPE_B, getCategoryLabel } from '../../lib/logic/wearables';
import { toWearableWithBlobs } from '../AssetPreview/utils';
import { WearablePreview } from '../WearablePreview';

import styles from './WearableViewer.module.css';

type Props = {
  file: File;
  /** The slot from the file name; a rigged model without one previews as upper body. */
  category?: WearableCategory;
  /** The slot the model's mesh names suggest, for a file whose name gives none. */
  suggestedCategory?: WearableCategory;
};

const BODY_SHAPES = [
  { urn: BODY_SHAPE_A, label: 'Body A' },
  { urn: BODY_SHAPE_B, label: 'Body B' },
] as const;

/** A wearable file on the default avatar, on either body shape. */
export function WearableViewer({ file, category, suggestedCategory }: Props) {
  const [bodyShape, setBodyShape] = useState<string>(BODY_SHAPE_B);
  const slot = category ?? suggestedCategory ?? 'upper_body';
  const blob = useMemo(
    () => toWearableWithBlobs(file, [], slot as SchemaWearableCategory),
    [file, slot],
  );

  return (
    <div className={`ModelViewer ${styles.root}`}>
      <WearablePreview
        id={`wearable-${file.name}`}
        blob={blob}
        profile="default"
        bodyShape={bodyShape as React.ComponentProps<typeof WearablePreview>['bodyShape']}
        disableBackground
        wheelZoom={2}
      />
      <div
        className={styles.bodies}
        role="group"
        aria-label="Body shape"
      >
        {BODY_SHAPES.map(option => (
          <button
            key={option.urn}
            type="button"
            className={styles.body}
            aria-pressed={bodyShape === option.urn}
            onClick={() => setBodyShape(option.urn)}
          >
            {option.label}
          </button>
        ))}
      </div>
      <span className={styles.badge}>
        {category
          ? `Wearable · ${getCategoryLabel(category)}`
          : suggestedCategory
            ? `Wearable · looks like ${getCategoryLabel(suggestedCategory).toLowerCase()}, no slot in its name`
            : 'Wearable · no slot in its name'}
      </span>
    </div>
  );
}
