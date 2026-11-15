import { useCallback } from 'react';
import { Toast, ToastHost } from '@ui/atoms/Toast';

import type { Notification } from '/@/modules/store/snackbar/types';
import { useSnackbar } from '/@/hooks/useSnackbar';
import { useEditor } from '/@/hooks/useEditor';
import { useExternalAgentAccess } from '/@/hooks/useExternalAgentAccess';
import { MissingScenes } from './MissingScenes';
import { Generic } from './Generic';
import { NewDependencyVersion } from './DependencyVersion';
import { SdkSkills } from './SdkSkills';
import { Deploy } from './Deploy';
import { ExternalAgentAccess } from './ExternalAgentAccess';

import styles from './component.module.css';

const DEFAULT_DURATION_IN_MS = 5_000;
const SLOT_HEIGHT_IN_PX = 60;

export function SnackbarComponent() {
  const { notifications, close, dismiss } = useSnackbar();
  const { isInstallingProject: isInstallingDependencies } = useEditor();
  // Mounted once for the whole app, like this component; its only output is a notice here.
  useExternalAgentAccess();

  const getComponent = useCallback((notification: Notification) => {
    switch (notification.type) {
      case 'generic':
        return (
          <Generic
            {...notification}
            onClose={notification.duration === 0 ? close(notification.id) : undefined}
          />
        );
      case 'missing-scenes':
        return <MissingScenes onClose={close(notification.id)} />;
      case 'new-dependency-version':
        return <NewDependencyVersion onClose={close(notification.id)} />;
      case 'sdk-skills':
        return (
          <SdkSkills
            onClose={close(notification.id)}
            project={notification.project}
            installed={notification.installed}
          />
        );
      case 'deploy':
        return (
          <Deploy
            onClose={close(notification.id)}
            path={notification.path}
          />
        );
      case 'external-agent-access':
        return <ExternalAgentAccess onClose={close(notification.id)} />;
      default:
        return null;
    }
  }, []);

  const getDuration = useCallback(({ duration }: Notification) => {
    if (duration === 0) return null;
    return duration || DEFAULT_DURATION_IN_MS;
  }, []);

  return (
    <>
      {isInstallingDependencies && (
        <div
          // The capture scenarios find the backdrop by the second class name.
          className={`${styles.backdrop} MuiBackdrop-root`}
          aria-hidden="true"
        />
      )}
      <ToastHost className="Snackbar">
        {notifications.map((notification, idx) => {
          const bottom = (notifications.length - idx) * SLOT_HEIGHT_IN_PX;
          const duration = getDuration(notification);

          return (
            <div
              // The capture scenarios find each notice by the second class name and read its bottom.
              className={`${styles.slot} MuiSnackbar-root`}
              style={{ bottom: `${bottom}px` }}
              key={notification.id}
            >
              <Toast
                role="presentation"
                duration={duration}
                // As the ui2 notice: the time stops while the window is not active, and a stop that ends
                // gives half of the duration again.
                holdOnWindowBlur
                resumeDuration={duration === null ? undefined : duration / 2}
                onClose={dismiss(notification.id, idx)}
              >
                {getComponent(notification)}
              </Toast>
            </div>
          );
        })}
      </ToastHost>
    </>
  );
}
