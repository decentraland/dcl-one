import { CheckCircleIcon, WarningAmberIcon } from '@ui/atoms/icons-material';
import type { Hex } from 'viem';

import { config } from '/@/config';
import type { TxFlow, TxPhase } from '/@/hooks/useLandTxFlow';
import { t } from '/@/modules/store/translation/utils';
import type { TranslationPath } from '/@/modules/store/translation/types';

import { DialogButton } from '../../DialogButton';
import { ExternalLink } from '../../ExternalLink';
import { ClockIcon, WarningCircleIcon } from '../../Icons';
import { Loader } from '../../Loader';
import modal from '../DefaultModal/DefaultModal.module.css';

import styles from './TxState.module.css';

const SEPOLIA = 11155111;

/** Etherscan page of a transaction on the given chain. */
export function getExplorerTxUrl(hash: Hex, chainId: number): string {
  return `https://${chainId === SEPOLIA ? 'sepolia.' : ''}etherscan.io/tx/${hash}`;
}

type Tone = 'progress' | 'success' | 'warning' | 'error';
type ActionSpec = { label: TranslationPath; action: 'close' | 'retry' };

const PHASES: Record<
  TxPhase,
  {
    text?: TranslationPath;
    tone: Tone;
    link: boolean;
    secondary?: ActionSpec;
    primary?: ActionSpec;
  }
> = {
  signing: {
    text: 'tx.waiting_signature',
    tone: 'progress',
    link: false,
    secondary: { label: 'modal.cancel', action: 'close' },
  },
  pending: {
    text: 'tx.pending',
    tone: 'progress',
    link: true,
    secondary: { label: 'modal.close', action: 'close' },
  },
  done: { tone: 'success', link: true, primary: { label: 'modal.close', action: 'close' } },
  unconfirmed: {
    text: 'tx.not_confirmed',
    tone: 'warning',
    link: true,
    primary: { label: 'modal.close', action: 'close' },
  },
  rejected: {
    text: 'tx.rejected',
    tone: 'warning',
    link: false,
    secondary: { label: 'modal.cancel', action: 'close' },
    primary: { label: 'tx.try_again', action: 'retry' },
  },
  expired: {
    text: 'tx.expired',
    tone: 'warning',
    link: false,
    secondary: { label: 'modal.cancel', action: 'close' },
    primary: { label: 'tx.try_again', action: 'retry' },
  },
  failed: {
    text: 'tx.failed',
    tone: 'error',
    link: true,
    secondary: { label: 'modal.close', action: 'close' },
    primary: { label: 'tx.try_again', action: 'retry' },
  },
};

const UNCONFIRMED_STEP: (typeof PHASES)[TxPhase] = {
  ...PHASES.unconfirmed,
  secondary: { label: 'modal.close', action: 'close' },
  primary: { label: 'tx.continue', action: 'retry' },
};

const GLYPHS: Record<Tone, JSX.Element> = {
  progress: <Loader size={24} />,
  success: <CheckCircleIcon aria-hidden />,
  warning: <WarningAmberIcon aria-hidden />,
  error: <WarningCircleIcon />,
};

type Props = {
  flow: TxFlow;
  doneText: string;
  stepLabels?: string[];
  onRetry: () => void;
  onClose: () => void;
};

const TERMINAL = new Set<TxPhase>(['done', 'unconfirmed']);

/** Where a tx modal offers Close: the corner ✕, or only TxState's own action once the tx has ended. */
export const txCorner = (flow: TxFlow | null): 'close' | 'none' =>
  flow && TERMINAL.has(flow.phase) ? 'none' : 'close';

/** The transaction state block shown inside a DefaultModal once an on-chain action is confirmed. */
export function TxState({ flow, doneText, stepLabels, onRetry, onClose }: Props) {
  const hasNextStep = !!flow.step && flow.step.n < flow.step.total;
  const phase = flow.phase === 'unconfirmed' && hasNextStep ? UNCONFIRMED_STEP : PHASES[flow.phase];
  const glyph = flow.phase === 'unconfirmed' ? <ClockIcon /> : GLYPHS[phase.tone];
  const handlers = { close: onClose, retry: onRetry };

  return (
    <div className={styles.root}>
      {flow.step && (
        <span className={styles.step}>
          {t('tx.step', { n: flow.step.n, total: flow.step.total })}
          {stepLabels?.[flow.step.n - 1] && ` · ${stepLabels[flow.step.n - 1]}`}
        </span>
      )}
      <div
        className={styles.stateRow}
        data-tone={phase.tone}
      >
        {glyph}
        <span
          role="status"
          aria-live="polite"
        >
          {phase.text ? t(phase.text) : doneText}
        </span>
      </div>
      {phase.link && flow.hash && (
        <ExternalLink
          className={styles.link}
          href={getExplorerTxUrl(flow.hash, Number(config.get('CHAIN_ID')))}
        >
          {t('tx.view_explorer')}
        </ExternalLink>
      )}
      <div className={modal.actions}>
        {phase.secondary && (
          <DialogButton
            type="button"
            tone="secondary"
            onClick={handlers[phase.secondary.action]}
          >
            {t(phase.secondary.label)}
          </DialogButton>
        )}
        {phase.primary && (
          <DialogButton
            type="button"
            tone="primary"
            autoFocus={phase.primary.action === 'retry'}
            onClick={handlers[phase.primary.action]}
          >
            {t(phase.primary.label)}
          </DialogButton>
        )}
      </div>
    </div>
  );
}
