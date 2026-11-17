import Spinner from '@ui/atoms/HubSpinner';

import { type GenericNotification } from '/@/modules/store/snackbar/types';

import { SnackbarAlert } from '../Alert';

import styles from './Generic.module.css';

type Props = GenericNotification & { onClose?: () => void };

export function Generic({ severity, message, description, onClose }: Props) {
  const props =
    severity === 'loading'
      ? {
          icon: (
            <Spinner
              ring="wide"
              role="progressbar"
              size={20}
              color="primary"
            />
          ),
        }
      : { severity };
  // Carries the severity so a test can assert an error was surfaced, not merely some
  // notification. The snackbar layer around it is presentational and would break on a
  // style refactor.
  const testId = `snackbar-generic-${severity}`;

  if (description) {
    return (
      <SnackbarAlert
        {...props}
        className={styles.detailed}
        data-testid={testId}
        // The capture scenarios find the title by this class name.
        title={<span className="MuiAlertTitle-root">{message}</span>}
        onClose={onClose}
      >
        <pre className={styles.description}>{description}</pre>
      </SnackbarAlert>
    );
  }

  return (
    <SnackbarAlert
      {...props}
      data-testid={testId}
    >
      {message}
    </SnackbarAlert>
  );
}
