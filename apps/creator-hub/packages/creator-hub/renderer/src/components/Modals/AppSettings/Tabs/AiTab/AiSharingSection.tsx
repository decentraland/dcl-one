import { useEffect, useId, useState } from 'react';
import Button from '@ui/atoms/Button';
import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/Spinner';
import Text from '@ui/atoms/Text';
import { Tooltip } from '@dcl/one-ui';
import { ChevronRightIcon, DeleteOutlineIcon } from '@ui/atoms/icons-material';

import { hasSharingConsent } from '/shared/types/ai-sharing';

import { useDispatch, useSelector } from '#store';
import { useAuth } from '/@/hooks/useAuth';
import {
  actions as aiSharingActions,
  canManageSharing,
  isSharingFeatureOn,
  selectSharingConsent,
} from '/@/modules/store/aiSharing';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';

import { ToggleRow } from '../../ToggleRow';

import styles from './AiSharingSection.module.css';

const PREVIEW_ROWS = 5;

const day = (iso: string) => new Date(iso).toLocaleDateString(undefined, { dateStyle: 'medium' });

// The service may add kinds and outcomes before this app knows them: show those as they come.
const translated = (key: string, fallback: string) =>
  getCurrentLocale().messages[key] !== undefined
    ? t(key as TranslationPath)
    : fallback.replace(/_/g, ' ');

const when = (iso: string) =>
  new Date(iso).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });

/**
 * Settings → AI: the sharing switch, what is waiting to upload, what has been shared, and
 * deleting it. Owns its state through the store (not the tab's settings props), because turning
 * sharing off must also empty the local queue and tell the service, which a plain settings write
 * can't do. Shown wherever sharing is offered, and wherever the signed-in wallet has a decision
 * stored, so withdrawing and erasing stay possible after the feature is turned off.
 */
export function AiSharingSection() {
  const dispatch = useDispatch();
  const { wallet, signIn } = useAuth();
  const address = wallet?.toLowerCase() ?? null;
  const visible = useSelector(state => canManageSharing(state, address));
  const offered = useSelector(isSharingFeatureOn);
  const consent = useSelector(state => selectSharingConsent(state, address));
  const { queued, deleteStatus, deletedCount, shared, digests } = useSelector(
    state => state.aiSharing,
  );
  const sharing = hasSharingConsent(consent);
  const [showShared, setShowShared] = useState(false);
  const [showDigests, setShowDigests] = useState(false);
  const sharedListId = useId();
  const digestsListId = useId();

  useEffect(() => {
    if (!visible) return;
    // Uploads read the queue again only while a view shows it.
    dispatch(aiSharingActions.showQueue());
    void dispatch(aiSharingActions.refreshQueue());
    return () => {
      dispatch(aiSharingActions.hideQueue());
      dispatch(aiSharingActions.resetDelete());
    };
  }, [visible, dispatch]);

  useEffect(() => {
    if (showShared && address !== null) {
      void dispatch(aiSharingActions.loadSharedTurns({ reset: true }));
    }
  }, [showShared, address, dispatch]);

  useEffect(() => {
    if (showDigests && address !== null) {
      void dispatch(aiSharingActions.loadDigests({ reset: true }));
    }
  }, [showDigests, address, dispatch]);

  if (!visible) return null;
  const queuedHere = queued.filter(item => item.address === address);
  const signedOut = address === null;
  const label = (key: string) =>
    t(`modal.app_settings.fields.ai_sharing.${key}` as TranslationPath);

  return (
    <>
      <hr className="AiDivider" />
      <div className="AiField">
        <div className={signedOut ? `${styles.toggle} ${styles.dimmed}` : styles.toggle}>
          <ToggleRow
            checked={sharing}
            disabled={signedOut || (!sharing && !offered)}
            label={label('label')}
            onChange={checked => void dispatch(aiSharingActions.decide(checked))}
          />
        </div>
        <Text
          variant="body2"
          tone="secondary"
          className={`AiHint ${styles.description} ${signedOut ? styles.dimmed : ''}`}
        >
          {label('description')}
        </Text>

        {signedOut ? (
          <p className={`AiHint ${styles.signin}`}>
            {label('signin')}
            <button
              type="button"
              className={`AgentSignLink primary ${styles.signinLink}`}
              onClick={() => void signIn()}
            >
              {label('signin_link')}
            </button>
          </p>
        ) : (
          <div className={styles.body}>
            {sharing && (
              <div className={styles.lines}>
                <p className={styles.title}>
                  {queuedHere.length > 0
                    ? t('modal.app_settings.fields.ai_sharing.queued_title', {
                        count: queuedHere.length,
                      })
                    : label('queued_empty')}
                </p>
                {queuedHere.slice(0, PREVIEW_ROWS).map(item => (
                  <p
                    key={item.recordId}
                    className={styles.line}
                  >
                    {when(item.startedAt)} · {item.promptPreview}
                  </p>
                ))}
              </div>
            )}

            {deleteStatus === 'confirming' ? (
              <div
                role="group"
                aria-label={label('delete')}
                className={styles.confirm}
              >
                <p className={styles.question}>{label('delete_confirm')}</p>
                <div className={styles.buttons}>
                  <Button
                    size="sm"
                    tone="danger"
                    onClick={() => void dispatch(aiSharingActions.deleteEverything())}
                  >
                    {label('delete_confirm_yes')}
                  </Button>
                  <Button
                    variant="secondary"
                    tone="neutral"
                    size="sm"
                    onClick={() => dispatch(aiSharingActions.resetDelete())}
                  >
                    {label('delete_cancel')}
                  </Button>
                </div>
              </div>
            ) : deleteStatus === 'deleted' ? (
              <p
                className={styles.line}
                role="status"
              >
                {t('modal.app_settings.fields.ai_sharing.deleted', { count: deletedCount ?? 0 })}
              </p>
            ) : (
              <>
                <div className={styles.row}>
                  <ListToggle
                    open={showShared}
                    controls={sharedListId}
                    onToggle={() => setShowShared(open => !open)}
                  >
                    {showShared ? label('viewer_hide') : label('viewer_show')}
                  </ListToggle>
                  <button
                    type="button"
                    className={`AgentSignLink ${styles.deleteLink}`}
                    disabled={deleteStatus === 'deleting'}
                    onClick={() => dispatch(aiSharingActions.askDeleteConfirmation())}
                  >
                    {deleteStatus === 'deleting' ? label('deleting') : label('delete')}
                  </button>
                </div>
                {deleteStatus === 'failed' && (
                  <p
                    className={styles.error}
                    role="alert"
                  >
                    {label('delete_failed')}
                  </p>
                )}
              </>
            )}

            {showShared && deleteStatus !== 'deleted' && (
              <div
                id={sharedListId}
                className={styles.list}
              >
                {shared.status === 'failed' && (
                  <p
                    className={styles.error}
                    role="alert"
                  >
                    {label('viewer_failed')}
                  </p>
                )}
                {shared.status === 'loaded' && shared.turns.length === 0 && (
                  <p className={styles.line}>{label('viewer_empty')}</p>
                )}
                {shared.turns.map(turn => (
                  <div
                    key={turn.recordId}
                    className={styles.entry}
                  >
                    <p className={`${styles.line} ${styles.gray}`}>
                      {when(turn.startedAt)} · {turn.prompt.text}
                    </p>
                    <Tooltip label={label('viewer_delete')}>
                      <span>
                        <IconButton
                          size="small"
                          className={styles.iconButton}
                          aria-label={label('viewer_delete')}
                          disabled={shared.erasing.includes(turn.recordId)}
                          onClick={() =>
                            void dispatch(aiSharingActions.eraseSharedTurn(turn.recordId))
                          }
                        >
                          <DeleteOutlineIcon fontSize="inherit" />
                        </IconButton>
                      </span>
                    </Tooltip>
                  </div>
                ))}
                {shared.status === 'loading' && <Spinner size={14} />}
                {shared.next !== null && shared.status === 'loaded' && (
                  <button
                    type="button"
                    className={`AgentSignLink ${styles.more}`}
                    onClick={() =>
                      void dispatch(aiSharingActions.loadSharedTurns({ reset: false }))
                    }
                  >
                    {label('viewer_more')}
                  </button>
                )}
              </div>
            )}

            {deleteStatus !== 'deleted' && (
              <div className={styles.row}>
                <ListToggle
                  open={showDigests}
                  controls={digestsListId}
                  onToggle={() => setShowDigests(open => !open)}
                >
                  {showDigests ? label('digests_hide') : label('digests_show')}
                </ListToggle>
              </div>
            )}
            {showDigests && deleteStatus !== 'deleted' && (
              <div
                id={digestsListId}
                className={styles.list}
              >
                {digests.status === 'failed' && (
                  <p
                    className={styles.error}
                    role="alert"
                  >
                    {label('digests_failed')}
                  </p>
                )}
                {digests.status === 'loaded' && digests.digests.length === 0 && (
                  <p className={styles.line}>{label('digests_empty')}</p>
                )}
                {digests.digests.map(digest => {
                  const kinds = [...new Set(digest.frictions.map(f => f.kind))];
                  return (
                    <div key={digest.conversationId}>
                      <p className={`${styles.line} ${styles.gray}`}>
                        {day(digest.createdAt)} · {digest.goal} ·{' '}
                        {translated(
                          `modal.app_settings.fields.ai_sharing.digest_outcome.${digest.outcome}`,
                          digest.outcome,
                        )}
                      </p>
                      {kinds.length > 0 && (
                        <p className={styles.line}>
                          {t('modal.app_settings.fields.ai_sharing.digests_problems', {
                            kinds: kinds
                              .map(kind =>
                                translated(
                                  `modal.app_settings.fields.ai_sharing.digest_friction.${kind}`,
                                  kind,
                                ),
                              )
                              .join(', '),
                          })}
                        </p>
                      )}
                    </div>
                  );
                })}
                {digests.status === 'loading' && <Spinner size={14} />}
                {digests.next !== null && digests.status === 'loaded' && (
                  <button
                    type="button"
                    className={`AgentSignLink ${styles.more}`}
                    onClick={() => void dispatch(aiSharingActions.loadDigests({ reset: false }))}
                  >
                    {label('digests_more')}
                  </button>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </>
  );
}

/** "SHOW WHAT I'VE SHARED" and its kin: a chevron and a caps label that open a list below. */
function ListToggle({
  open,
  controls,
  onToggle,
  children,
}: {
  open: boolean;
  controls: string;
  onToggle: () => void;
  children: string;
}) {
  return (
    <button
      type="button"
      className={`AgentSignLink ${styles.toggleLink}`}
      aria-expanded={open}
      aria-controls={controls}
      onClick={onToggle}
    >
      <ChevronRightIcon
        className={styles.chevron}
        data-open={open}
      />
      {children}
    </button>
  );
}
