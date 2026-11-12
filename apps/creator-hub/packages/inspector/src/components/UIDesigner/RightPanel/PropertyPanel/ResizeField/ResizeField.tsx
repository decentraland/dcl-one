import React from 'react';
import cx from 'classnames';
import type { Entity } from '@dcl/ecs';

import { useAppDispatch, useAppSelector } from '../../../../../redux/hooks';
import { getAspectLockedNodes, setAspectLocked } from '../../../../../redux/ui-designer';
import { Dropdown, TextField } from '../../../../ui';
import { BindableSubField } from '../BindableSubField';
import type { FieldConfig } from '../field-configs';
import {
  axisForPath,
  convertLength,
  measureNodeBox,
  measureParentBox,
} from '../../../shared/measure';
import {
  MODE_LABELS,
  type ResizeAxis,
  type ResizeMode,
  mainAxisFor,
  resizeMode,
  resizeModesFor,
  resizePatch,
  resizeValuePatch,
} from '../resize-modes';
import { YGU_PERCENT, YGU_POINT } from '../../../../../lib/sdk/ui-transform-constants';

import './ResizeField.css';

interface ResizeFieldProps {
  field: FieldConfig;
  value: Record<string, unknown> | null;
  entity: Entity;
  bindings?: Record<string, string>;
  parentFlexDirection: number;
  overriding?: boolean;
  onPatch: (patch: Record<string, unknown>) => void;
}

const UNIT_OF: Partial<Record<ResizeMode, number>> = {
  fixed: YGU_POINT,
  percent: YGU_PERCENT,
};

const UNIT_LABEL: Partial<Record<ResizeMode, string>> = {
  fixed: 'px',
  percent: '%',
};

export const ResizeField: React.FC<ResizeFieldProps> = ({
  field,
  value,
  entity,
  bindings,
  parentFlexDirection,
  overriding,
  onPatch,
}) => {
  const dispatch = useAppDispatch();
  const aspectLockedMap = useAppSelector(getAspectLockedNodes);
  const aspectLocked = !!field.aspectLockable && !!aspectLockedMap[entity as unknown as number];
  const mainAxis = mainAxisFor(parentFlexDirection);

  const patchValue = (axis: ResizeAxis, mode: ResizeMode, next: number) => {
    const patch = resizeValuePatch(axis, mode, next);
    const other: ResizeAxis = axis === 'width' ? 'height' : 'width';
    const otherMode = resizeMode(value, other, mainAxis);
    const cur = (value?.[axis] as number | undefined) ?? 0;
    const curOther = (value?.[other] as number | undefined) ?? 0;
    if (aspectLocked && UNIT_OF[otherMode] && cur > 0 && curOther > 0) {
      Object.assign(
        patch,
        resizeValuePatch(other, otherMode, Math.max(0, Math.round(next * (curOther / cur)))),
      );
    }
    onPatch(patch);
  };

  return (
    <div className="ui-designer-resize">
      {(field.subFields ?? []).map(sub => {
        const axis = sub.path as ResizeAxis;
        const mode = resizeMode(value, axis, mainAxis);
        const modes = resizeModesFor(value, { overriding, current: mode });
        const numeric = (value?.[axis] as number | undefined) ?? 0;
        const sized = !!UNIT_OF[mode];
        const rendered = sized ? undefined : measureNodeBox(entity)?.[axis];
        const shown = rendered === undefined ? numeric : Math.round(rendered);
        return (
          <BindableSubField
            key={axis}
            field={{ componentId: field.componentId, path: axis, kind: 'length' }}
            entity={entity}
            bound={bindings?.[`${field.componentId}.${axis}`]}
          >
            <TextField
              type="number"
              leftLabel={sub.leftLabel}
              value={String(shown)}
              readOnly={!sized}
              rightContent={
                <span
                  className={cx('ui-designer-unit-control', {
                    'is-mode-word': !UNIT_LABEL[mode],
                  })}
                >
                  <Dropdown
                    options={modes.map(m => ({
                      value: m,
                      label: MODE_LABELS[m],
                      selectedLabel: UNIT_LABEL[m],
                    }))}
                    value={mode}
                    aria-label={`${sub.leftLabel === 'W' ? 'Width' : 'Height'} resize mode`}
                    onChange={e => {
                      const next = (e.target as HTMLSelectElement).value as ResizeMode;
                      if (next === mode) return;
                      const parent = measureParentBox(entity);
                      const dim = parent ? parent[axisForPath(axis)] : 0;
                      onPatch(
                        resizePatch({
                          next,
                          current: mode,
                          axis,
                          mainAxis,
                          value: convertLength(
                            numeric,
                            UNIT_OF[mode] ?? YGU_POINT,
                            UNIT_OF[next] ?? YGU_POINT,
                            dim,
                          ),
                        }),
                      );
                    }}
                  />
                </span>
              }
              onChange={e => patchValue(axis, mode, Number(e.target.value) || 0)}
            />
          </BindableSubField>
        );
      })}
      {field.aspectLockable ? (
        <button
          type="button"
          className="ui-designer-vec-lock ui-designer-toggle"
          aria-pressed={aspectLocked}
          aria-label={aspectLocked ? 'Unlock aspect ratio' : 'Lock aspect ratio'}
          title={aspectLocked ? 'Unlock aspect ratio' : 'Lock aspect ratio'}
          onClick={() => dispatch(setAspectLocked({ entity, locked: !aspectLocked }))}
        />
      ) : null}
    </div>
  );
};

export default ResizeField;
