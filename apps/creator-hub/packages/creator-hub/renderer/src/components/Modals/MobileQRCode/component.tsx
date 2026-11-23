import { useEffect, useState } from 'react';
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

  const connected = sessions.length > 0;
  useEffect(() => {
    if (connected) void analytics.track('Mobile Preview Connected', { devices: sessions.length });
  }, [connected]);

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
