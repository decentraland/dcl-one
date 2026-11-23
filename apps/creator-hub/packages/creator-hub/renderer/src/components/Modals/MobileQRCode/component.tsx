import { useEffect, useRef, useState } from 'react';
import { analytics, editor } from '#preload';
import type { MobileDebugSessionInfo } from '/shared/types/ipc';

import { t } from '/@/modules/store/translation/utils';
import { Modal } from '..';
import type { Props } from './types';

import './styles.css';

export function MobileQRCode({ open, onClose, url, qr }: Props) {
  const [sessions, setSessions] = useState<MobileDebugSessionInfo[]>([]);

  useEffect(() => {
    if (!open) {
      setSessions([]);
      return;
    }
    const unsubscribe = editor.onMobileDebugSessions(setSessions);
    return unsubscribe;
  }, [open]);

  // One event per device as it connects, with how many are connected then. Main keeps a
  // session in the list after its device disconnects.
  const reported = useRef(new Set<MobileDebugSessionInfo['id']>());
  useEffect(() => {
    const live = sessions.filter(session => !session.disconnectedAt);
    for (const session of live) {
      if (reported.current.has(session.id)) continue;
      reported.current.add(session.id);
      void analytics.track('Mobile Preview Connected', { devices: live.length });
    }
  }, [sessions]);

  return (
    <Modal
      className="MobileQRCodeModal"
      open={open}
      size="tiny"
      title={t('modal.mobile_qr.title')}
      subtitle={t('modal.mobile_qr.description')}
      onClose={onClose}
    >
      <div className="MobileQRCodeContent">
        <div className="QRContainer">
          <img
            src={qr}
            alt="QR Code"
            className="QRImage"
          />
        </div>
        <span className="Url">{url}</span>
        <div className="SessionsContainer">
          {sessions.length === 0 ? (
            <span className="SessionStatus waiting">Waiting for mobile connection...</span>
          ) : (
            sessions.map(s => (
              <div
                key={s.id}
                className="SessionItem"
              >
                <span className="SessionBadge connected">Session #{s.id}</span>
                <span className="SessionMessages">{s.messageCount.toLocaleString()} entries</span>
              </div>
            ))
          )}
        </div>
        <span className="Disclaimer">{t('modal.mobile_qr.disclaimer')}</span>
      </div>
    </Modal>
  );
}
