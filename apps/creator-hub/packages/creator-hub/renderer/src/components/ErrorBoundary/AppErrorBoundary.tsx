import { type ReactNode } from 'react';
import Button from '@ui/atoms/Button';

import { ErrorBoundary } from './component';
import { ErrorFallback, fallbackText } from './ErrorFallback';

type Props = {
  /** Names the window in the Sentry `boundary` tag, e.g. `app` or `console-window`. */
  name: string;
  children: ReactNode;
};

/**
 * Last-resort boundary around a whole window's tree. It sits outside the store, router,
 * theme and translation providers, so its fallback takes its look from the global tokens,
 * reads English when no locale is loaded, and recovers by remounting or reloading instead of routing.
 */
export function AppErrorBoundary({ name, children }: Props) {
  return (
    <ErrorBoundary
      name={name}
      fallback={({ reset }) => (
        <ErrorFallback
          description={fallbackText('error_boundary.app_description')}
          testId="app-error-fallback"
        >
          <Button
            variant="primary"
            tone="neutral"
            onClick={() => window.location.reload()}
          >
            {fallbackText('error_boundary.reload')}
          </Button>
          <Button
            variant="primary"
            onClick={reset}
          >
            {fallbackText('error_boundary.try_again')}
          </Button>
        </ErrorFallback>
      )}
    >
      {children}
    </ErrorBoundary>
  );
}
