import type { PBInputModifier } from '@dcl/ecs';

export type StandardInput = NonNullable<PBInputModifier['mode']>['standard'];
export type BlockKey = Exclude<keyof StandardInput, 'disableAll'>;

/** What the player can be stopped from doing, in the order the docs list them. */
export const BLOCKABLE: { key: BlockKey; label: string }[] = [
  { key: 'disableWalk', label: 'Walk' },
  { key: 'disableJog', label: 'Jog' },
  { key: 'disableRun', label: 'Run' },
  { key: 'disableJump', label: 'Jump' },
  { key: 'disableDoubleJump', label: 'Double jump' },
  { key: 'disableGliding', label: 'Glide' },
  { key: 'disableEmote', label: 'Emotes' },
];

export function getStandardInput(value: PBInputModifier | null | undefined): StandardInput {
  return value?.mode?.$case === 'standard' ? value.mode.standard : {};
}

export function setStandardInput(
  value: PBInputModifier | null | undefined,
  key: keyof StandardInput,
  checked: boolean,
): PBInputModifier {
  const standard = { ...getStandardInput(value), [key]: checked };
  return { ...value, mode: { $case: 'standard', standard } };
}
