import { useState } from 'react';
import { cx } from '/@/lib/cx';

import Button from '@ui/atoms/HubButton';
import { Select, SelectOption } from '@ui/atoms/Select';
import Tooltip from '@ui/atoms/Tooltip';

import type { CreatorFundPlace, PlaceReason } from '/shared/types/creator-fund';

import { misc } from '#preload';
import { getSceneJumpInDeeplink } from '/@/modules/utils';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import {
  ArrowDownIcon,
  CheckmarkIcon,
  InProgramIcon,
  JumpInIcon,
  MetricInfoIcon,
  NotEligibleIcon,
} from '../icons';
import { Spinner } from '../Spinner';

import modal from '../EarnModal.module.css';
import landing from '../Landing.module.css';

import { jumpInLocation, placeTitle } from './format';
import styles from './parts.module.css';

const REASONS: Record<PlaceReason, TranslationPath> = {
  content_policy: 'earn.joined.my_places.reasons.content_policy',
  excluded: 'earn.joined.my_places.reasons.excluded',
  not_owner: 'earn.joined.my_places.reasons.not_owner',
  creator_not_eligible: 'earn.joined.my_places.reasons.creator_not_eligible',
  terms_outdated: 'earn.joined.my_places.reasons.terms_outdated',
};

const reasonLines = (reasons: string[]) => [
  ...new Set(
    (reasons.length ? reasons : ['unknown']).map(
      reason => REASONS[reason as PlaceReason] ?? 'earn.joined.my_places.reasons.unknown',
    ),
  ),
];

/**
 * The look and place of EARN tooltips: over the anchor, with an arrow. Give the tooltip its anchor
 * element: it then describes that element while it is open, and adds no element around it.
 */
export const EARN_TOOLTIP = {
  side: 'top',
  arrow: true,
  keepInViewport: true,
  maxWidth: 340,
  enterDelay: 100,
  tipClassName: styles.tooltip,
} as const;

export type ChipProps =
  | { status: 'in_program' }
  | { status: 'not_eligible'; reasons: string[] }
  | { status: 'replaced' };

function ExplainedChip({
  status,
  icon,
  title,
}: {
  status: 'not_eligible' | 'replaced';
  icon?: React.ReactNode;
  title: React.ReactNode;
}) {
  const [chip, setChip] = useState<HTMLSpanElement | null>(null);
  return (
    <Tooltip
      {...EARN_TOOLTIP}
      content={title}
      anchor={chip}
    >
      <span
        ref={setChip}
        className={styles.chip}
        data-status={status}
        tabIndex={0}
      >
        {icon}
        {t(`earn.joined.my_places.status.${status}`)}
      </span>
    </Tooltip>
  );
}

/** The program status of one of the user's places; the other statuses explain themselves in a tooltip. */
export function StatusChip(props: ChipProps) {
  if (props.status === 'in_program') {
    return (
      <span
        className={styles.chip}
        data-status="in_program"
      >
        <InProgramIcon />
        {t('earn.joined.my_places.status.in_program')}
      </span>
    );
  }
  if (props.status === 'replaced') {
    return (
      <ExplainedChip
        status="replaced"
        title={t('earn.joined.my_places.replaced_tooltip')}
      />
    );
  }
  return (
    <ExplainedChip
      status="not_eligible"
      icon={<NotEligibleIcon />}
      title={reasonLines(props.reasons).map(reason => (
        <span
          key={reason}
          className={styles.tooltipLine}
        >
          {t(reason)}
        </span>
      ))}
    />
  );
}

export type MetricReading = { status: 'loading' } | { status: 'ready'; value: string | null };

function MetricReadingValue({ reading }: { reading: MetricReading }) {
  if (reading.status === 'loading') {
    return (
      <dd
        className={styles.metricValue}
        aria-busy="true"
      >
        <span
          className={styles.metricSkeleton}
          aria-hidden
        />
        <span className={modal.visuallyHidden}>{t('earn.loading')}</span>
      </dd>
    );
  }
  return (
    <dd className={styles.metricValue}>
      {reading.value ?? (
        <>
          <span aria-hidden>-</span>
          <span className={modal.visuallyHidden}>{t('earn.joined.my_places.metrics.no_data')}</span>
        </>
      )}
    </dd>
  );
}

export type MetricExplanation = { name: string; tooltip: string };

function MetricInfo({ name, tooltip }: MetricExplanation) {
  const [button, setButton] = useState<HTMLButtonElement | null>(null);
  return (
    <Tooltip
      {...EARN_TOOLTIP}
      content={tooltip}
      anchor={button}
    >
      <button
        ref={setButton}
        className={cx(styles.infoButton, styles.metricInfoButton)}
        type="button"
        aria-label={name}
      >
        <MetricInfoIcon />
      </button>
    </Tooltip>
  );
}

/**
 * One metric of a place: a skeleton named "Loading" while it loads, "-" named "No data" when absent.
 * @param props.info - an ⓘ button after the label: its accessible name and its tooltip.
 */
export function MetricStat({
  icon,
  label,
  info,
  reading,
}: {
  icon: React.ReactNode;
  label: string;
  info?: MetricExplanation;
  reading: MetricReading;
}) {
  return (
    <div className={styles.metric}>
      <dt className={styles.metricLabel}>
        {icon}
        <span
          className={styles.metricLabelText}
          title={label}
        >
          {label}
        </span>
        {info && <MetricInfo {...info} />}
      </dt>
      <MetricReadingValue reading={reading} />
    </div>
  );
}

/**
 * Opens a place in the explorer: a scene in a World of several scenes at its own position.
 * @param props.scenesInWorld - the place's World scene count, when known.
 * @param props.onJumpIn - called before the explorer opens.
 */
export function JumpInButton({
  place,
  scenesInWorld = 0,
  onJumpIn,
}: {
  place: CreatorFundPlace;
  scenesInWorld?: number;
  onJumpIn?: () => void;
}) {
  return (
    <Button
      className={styles.jumpIn}
      aria-label={t('earn.joined.leaderboard.jump_in_label', { place: placeTitle(place) })}
      onClick={() => {
        onJumpIn?.();
        void misc.openExternal(
          getSceneJumpInDeeplink(jumpInLocation(place), { position: scenesInWorld > 1 }),
        );
      }}
    >
      {t('earn.joined.leaderboard.jump_in')}
      <JumpInIcon />
    </Button>
  );
}

export type ListStateProps =
  | { kind: 'loading' }
  | { kind: 'empty'; message: string }
  | { kind: 'error'; message: string; onRetry: () => void };

/** A list's loading, empty or error state, centred where the rows would be. */
export function ListState(props: ListStateProps) {
  return (
    <div
      className={styles.stateBox}
      data-kind={props.kind}
    >
      {props.kind === 'loading' && (
        <Spinner
          label={t('earn.loading')}
          size={48}
        />
      )}
      {props.kind === 'empty' && (
        <p
          className={landing.statusLine}
          role="status"
        >
          {props.message}
        </p>
      )}
      {props.kind === 'error' && (
        <>
          <p
            className={styles.errorLine}
            role="alert"
          >
            {props.message}
          </p>
          <Button
            className={styles.retryButton}
            variant="secondary"
            size="lg"
            onClick={props.onRetry}
          >
            {t('earn.errors.retry')}
          </Button>
        </>
      )}
    </div>
  );
}

export type CycleOption = { value: string; label: string };

/** The filter bar's "Cycle" select. */
export function CycleSelect({
  value,
  options,
  onChange,
}: {
  value: string;
  options: CycleOption[];
  onChange: (value: string) => void;
}) {
  const [keyboard, setKeyboard] = useState(false);
  return (
    <div className={styles.filterGroup}>
      <span
        className={styles.filterLabel}
        aria-hidden
      >
        {t('earn.joined.filter.cycle')}
      </span>
      <div
        onKeyDownCapture={() => setKeyboard(true)}
        onPointerDownCapture={() => setKeyboard(false)}
      >
        <Select
          className={styles.cycleSelect}
          menuClassName={cx(styles.cycleMenu, keyboard && styles.keyboardMenu)}
          variant="plain"
          value={value}
          onChange={onChange}
          renderValue={selected => options.find(option => option.value === selected)?.label}
          icon={<ArrowDownIcon />}
          placement="bottom-end"
          aria-label={t('earn.joined.filter.cycle')}
        >
          {options.map(option => (
            <SelectOption
              key={option.value}
              value={option.value}
              label={option.label}
            >
              {option.value === value && <CheckmarkIcon />}
              {option.label}
            </SelectOption>
          ))}
        </Select>
      </div>
    </div>
  );
}
