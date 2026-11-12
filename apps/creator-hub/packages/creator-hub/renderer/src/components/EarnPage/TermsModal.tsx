import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { Button, Checkbox, FormControlLabel, IconButton } from 'decentraland-ui2';

import { analytics } from '#preload';
import { useDispatch, useSelector } from '#store';
import { actions } from '/@/modules/store/creatorFund';
import { t } from '/@/modules/store/translation/utils';

import MarkdownRenderer from '../Modals/AppSettings/MarkdownRenderer/MarkdownRenderer';
import { Modal } from '../Modals';

import { openMarkdownLink } from './markdownLinks';
import { Spinner } from './Spinner';
import { CheckboxTickIcon, CloseIcon, InfoIcon } from './icons';
import modal from './EarnModal.module.css';
import styles from './TermsModal.module.css';

const BOTTOM_TOLERANCE_PX = 4;

const FLOW = { accept: 'join', reaccept: 'reaccept' } as const;

const ImageAltText = ({ alt }: { alt?: string }) => <>{alt}</>;
const TERMS_OVERRIDES = { img: { component: ImageAltText } };

export type TermsModalProps =
  | { variant: 'read'; onClose: () => void }
  | {
      variant: 'accept' | 'reaccept';
      onAccept: (termsVersion: number) => void;
      onClose: () => void;
      error?: string;
      status?: 'idle' | 'submitting';
    };

function formatUpdatedAt(updatedAt: string, locale: string) {
  return new Intl.DateTimeFormat(locale, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    timeZone: 'UTC',
  }).format(new Date(updatedAt));
}

/**
 * The Program Terms modal.
 * @param props.variant - `read`, or `accept` / `reaccept`, gated on scrolling to the bottom; `reaccept` cannot be dismissed.
 */
export function TermsModal(props: TermsModalProps) {
  const { variant, onClose } = props;
  const dispatch = useDispatch();
  const terms = useSelector(state => state.creatorFund.terms);
  const locale = useSelector(state => state.translation.locale);
  const titleId = useId();
  const hintId = useId();
  const scrollerRef = useRef<HTMLDivElement>(null);
  const [atBottom, setAtBottom] = useState(false);
  const [checked, setChecked] = useState(false);
  /*
   * The modal opens on the cached terms and refetches them. A newer version that arrives meanwhile
   * is different text, so the scroll gate and the checkbox start over for it.
   */
  const version = terms.data?.version;
  const [gateVersion, setGateVersion] = useState(version);
  if (version !== gateVersion) {
    setGateVersion(version);
    if (gateVersion !== undefined) {
      setAtBottom(false);
      setChecked(false);
    }
  }

  const loadTerms = useCallback(() => dispatch(actions.fetchTerms()), [dispatch]);
  useEffect(() => {
    void loadTerms();
  }, [loadTerms]);

  const checkBottom = useCallback(() => {
    const scroller = scrollerRef.current;
    if (
      scroller &&
      scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - BOTTOM_TOLERANCE_PX
    ) {
      setAtBottom(true);
    }
  }, []);

  useLayoutEffect(() => {
    if (terms.data) checkBottom();
  }, [terms.data, checkBottom]);

  useEffect(() => {
    window.addEventListener('resize', checkBottom);
    return () => window.removeEventListener('resize', checkBottom);
  }, [checkBottom]);

  const decline = useCallback(
    (via: 'not_now' | 'close' | 'escape') => {
      if (variant !== 'read') {
        void analytics.track('Creator Fund Terms Declined', {
          terms_version: terms.data?.version ?? null,
          flow: FLOW[variant],
          via,
        });
      }
      onClose();
    },
    [variant, terms.data, onClose],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Escape' && variant !== 'reaccept') decline('escape');
    },
    [variant, decline],
  );

  const handleAccept = useCallback(() => {
    if (props.variant === 'read' || !terms.data) return;
    void analytics.track('Creator Fund Terms Accepted', {
      terms_version: terms.data.version,
      flow: FLOW[props.variant],
    });
    props.onAccept(terms.data.version);
  }, [props, terms.data]);

  const heading =
    variant === 'reaccept'
      ? t('earn.reaccept.title')
      : (terms.data?.title ?? t('earn.terms.region'));
  const subtitle =
    variant === 'reaccept'
      ? t('earn.reaccept.subtitle')
      : terms.data &&
        t('earn.terms.updated_line', {
          date: formatUpdatedAt(terms.data.published_at, locale),
          version: terms.data.version,
        });

  return (
    <Modal
      className={modal.termsShell}
      open
    >
      <div
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={handleKeyDown}
      >
        <div className={styles.titleBar}>
          <div className={styles.titleText}>
            <h2
              className={styles.title}
              id={titleId}
            >
              {heading}
            </h2>
            {subtitle && <p className={styles.subtitle}>{subtitle}</p>}
          </div>
          {variant !== 'reaccept' && (
            <IconButton
              className={styles.titleBarClose}
              aria-label={t('earn.close')}
              onClick={() => decline('close')}
            >
              <CloseIcon />
            </IconButton>
          )}
        </div>
        <div
          className={styles.glow}
          aria-hidden
        />
        <div
          className={styles.scrollview}
          data-variant={variant}
        >
          <div
            className={styles.box}
            data-variant={variant}
          >
            <div
              className={styles.scroller}
              ref={scrollerRef}
              role="region"
              aria-label={t('earn.terms.region')}
              tabIndex={0}
              onScroll={checkBottom}
            >
              {terms.data ? (
                <div
                  className={styles.termsText}
                  onClick={openMarkdownLink}
                >
                  <MarkdownRenderer
                    content={terms.data.content}
                    overrides={TERMS_OVERRIDES}
                  />
                </div>
              ) : terms.status === 'failed' ? (
                <div className={styles.boxState}>
                  <span role="alert">{t('earn.terms.load_error')}</span>
                  <Button
                    className={modal.modalCta}
                    variant="contained"
                    color="secondary"
                    size="large"
                    onClick={loadTerms}
                  >
                    {t('earn.errors.retry')}
                  </Button>
                </div>
              ) : (
                <div className={styles.boxState}>
                  <Spinner label={t('earn.loading')} />
                </div>
              )}
            </div>
          </div>
          {variant !== 'read' && (
            <div className={styles.gate}>
              {atBottom ? (
                <FormControlLabel
                  control={
                    <Checkbox
                      color="primary"
                      disableRipple
                      icon={<span className={styles.tickBox} />}
                      checkedIcon={
                        <span
                          className={styles.tickBox}
                          data-checked
                        >
                          <CheckboxTickIcon />
                        </span>
                      }
                      checked={checked}
                      onChange={event => setChecked(event.target.checked)}
                    />
                  }
                  label={t('earn.terms.accept_checkbox')}
                />
              ) : (
                <>
                  <InfoIcon />
                  <span
                    className={styles.hint}
                    id={hintId}
                  >
                    {t('earn.terms.hint')}
                  </span>
                </>
              )}
              <span
                className={modal.visuallyHidden}
                aria-live="polite"
              >
                {atBottom ? t('earn.terms.gate_open') : ''}
              </span>
            </div>
          )}
          {props.variant !== 'read' && props.error && (
            <p
              className={modal.errorLine}
              role="alert"
            >
              {props.error}
            </p>
          )}
          <div className={modal.actions}>
            <Button
              className={modal.modalCta}
              variant="contained"
              color="secondary"
              size="large"
              onClick={() => decline('not_now')}
            >
              {variant === 'read' ? t('earn.terms.close') : t('earn.terms.not_now')}
            </Button>
            {props.variant !== 'read' && (
              <Button
                className={modal.modalCta}
                variant="contained"
                color="primary"
                size="large"
                disabled={!checked || !terms.data || props.status === 'submitting'}
                aria-busy={props.status === 'submitting' || undefined}
                aria-describedby={atBottom ? undefined : hintId}
                onClick={handleAccept}
              >
                {variant === 'reaccept'
                  ? t('earn.reaccept.accept')
                  : t('earn.terms.accept_and_join')}
              </Button>
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}
