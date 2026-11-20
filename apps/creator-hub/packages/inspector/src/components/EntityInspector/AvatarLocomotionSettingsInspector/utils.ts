import type { PBAvatarLocomotionSettings } from '@dcl/ecs';

export type LocomotionKey = keyof PBAvatarLocomotionSettings;

type Field = { key: LocomotionKey; label: string; fallback: number };

/** The settings by group, with the explorer defaults the docs list for each. */
export const LOCOMOTION_GROUPS: { label: string; unit: string; fields: Field[] }[] = [
  {
    label: 'Speed',
    unit: 'm/s',
    fields: [
      { key: 'walkSpeed', label: 'Walk', fallback: 1.5 },
      { key: 'jogSpeed', label: 'Jog', fallback: 8 },
      { key: 'runSpeed', label: 'Run', fallback: 10 },
    ],
  },
  {
    label: 'Jump height',
    unit: 'm',
    fields: [
      { key: 'jumpHeight', label: 'Jump', fallback: 1 },
      { key: 'runJumpHeight', label: 'Run', fallback: 1.5 },
      { key: 'doubleJumpHeight', label: 'Double', fallback: 2 },
    ],
  },
  {
    label: 'Gliding',
    unit: 'm/s',
    fields: [
      { key: 'glidingSpeed', label: 'Speed', fallback: 6 },
      { key: 'glidingFallingSpeed', label: 'Fall', fallback: 1 },
    ],
  },
  {
    label: 'Hard landing cooldown',
    unit: 's',
    fields: [{ key: 'hardLandingCooldown', label: 'Wait', fallback: 0.75 }],
  },
];

/**
 * `value` with `key` set from the typed `text`: empty unsets it so the explorer default applies,
 * and a negative number becomes 0, as explorers clamp it. Null when `text` is not a number.
 */
export function setLocomotionSetting(
  value: PBAvatarLocomotionSettings,
  key: LocomotionKey,
  text: string,
): PBAvatarLocomotionSettings | null {
  const next = { ...value };
  if (!text.trim()) {
    delete next[key];
    return next;
  }
  const number = Number(text);
  if (!Number.isFinite(number)) return null;
  next[key] = Math.max(0, number);
  return next;
}
