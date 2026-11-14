import React from 'react';
import type { Entity } from '@dcl/ecs';
import { BindableField } from '../BindableField';
import { BindableSubField } from '../BindableSubField';
import { Dropdown } from '../../../../ui';
import type { FieldConfig } from '../field-configs';
import { TextField } from '../../../../ui';
import { YGU_AUTO } from '../../../../../lib/sdk/ui-transform-constants';
import { YGU_POINT } from '../../../../../lib/sdk/ui-transform-constants';
import { YGU_UNDEFINED } from '../../../../../lib/sdk/ui-transform-constants';
import { axisForPath } from '../../../shared/measure';
import { convertLength } from '../../../shared/measure';
import { getAspectLockedNodes } from '../../../../../redux/ui-designer';
import { measureParentBox } from '../../../shared/measure';
import { setAspectLocked } from '../../../../../redux/ui-designer';
import { useAppDispatch } from '../../../../../redux/hooks';
import { useAppSelector } from '../../../../../redux/hooks';
import { UNIT_OPTIONS, clampNumber } from '../field-helpers';

interface LengthVecFieldProps {
  field: FieldConfig;
  componentValue: Record<string, unknown> | null;
  entity: Entity;
  bindings?: Record<string, string>;
  boundProp?: { variable: string };
  fieldDisabled: boolean;
  onPatch: (patch: Record<string, unknown>) => void;
}

export const LengthVecField = React.memo(function LengthVecField({
  field,
  componentValue,
  entity,
  bindings,
  boundProp,
  fieldDisabled,
  onPatch,
}: LengthVecFieldProps) {
  const dispatch = useAppDispatch();
  const aspectLockedMap = useAppSelector(getAspectLockedNodes);
  const aspectLocked = !!field.aspectLockable && !!aspectLockedMap[entity as unknown as number];
  const subs = field.facadeSubFields?.(componentValue) ?? field.subFields ?? [];

  const unitOf = (path: string) => {
    const raw = (componentValue?.[`${path}Unit`] as number | undefined) ?? YGU_UNDEFINED;
    return raw === YGU_UNDEFINED ? YGU_POINT : raw;
  };

  const switchUnit = (nextUnit: number) => {
    const parent = measureParentBox(entity);
    const patch: Record<string, unknown> = {};
    for (const sub of subs) {
      const cur = (componentValue?.[sub.path] as number | undefined) ?? 0;
      const dim = parent ? parent[axisForPath(sub.path)] : 0;
      patch[sub.path] = convertLength(cur, unitOf(sub.path), nextUnit, dim);
      patch[`${sub.path}Unit`] = nextUnit;
    }
    onPatch(patch);
  };

  return (
    <BindableField
      field={field}
      entity={entity}
      bound={boundProp}
    >
      {subs.map(sub => {
        const v = (componentValue?.[sub.path] as number | undefined) ?? 0;
        const unit = unitOf(sub.path);
        const subBound = bindings?.[`${field.componentId}.${sub.path}`];
        return (
          <BindableSubField
            key={sub.path}
            field={{ componentId: field.componentId, path: sub.path, kind: 'length' }}
            entity={entity}
            bound={subBound}
          >
            <TextField
              type="number"
              leftLabel={sub.leftLabel}
              value={String(v)}
              disabled={fieldDisabled || unit === YGU_AUTO}
              rightContent={
                <span className="ui-designer-unit-control">
                  <Dropdown
                    options={UNIT_OPTIONS}
                    value={unit}
                    aria-label={`${sub.leftLabel} unit`}
                    disabled={fieldDisabled}
                    onChange={e => switchUnit(Number(e.target.value))}
                  />
                </span>
              }
              onChange={e => {
                const next = clampNumber(e.target.value);
                const patch: Record<string, unknown> = {
                  [sub.path]: next,
                  [`${sub.path}Unit`]: unit,
                };
                if (aspectLocked && subs.length === 2) {
                  const other = subs.find(s => s.path !== sub.path);
                  const curThis = (componentValue?.[sub.path] as number | undefined) ?? 0;
                  const curOther = other
                    ? ((componentValue?.[other.path] as number | undefined) ?? 0)
                    : 0;
                  if (other && curThis > 0 && curOther > 0) {
                    patch[other.path] = Math.max(0, Math.round(next * (curOther / curThis)));
                    patch[`${other.path}Unit`] = unitOf(other.path);
                  }
                }
                onPatch(patch);
              }}
            />
          </BindableSubField>
        );
      })}
      {field.aspectLockable ? (
        <div className="ui-designer-unit-selector">
          <button
            type="button"
            className="ui-designer-vec-lock ui-designer-toggle"
            aria-pressed={aspectLocked}
            disabled={fieldDisabled}
            aria-label={aspectLocked ? 'Unlock aspect ratio' : 'Lock aspect ratio'}
            title={aspectLocked ? 'Unlock aspect ratio' : 'Lock aspect ratio'}
            onClick={() => dispatch(setAspectLocked({ entity, locked: !aspectLocked }))}
          />
        </div>
      ) : null}
    </BindableField>
  );
});
