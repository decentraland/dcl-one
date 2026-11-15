import type { ReactNode } from 'react';

/** The providers a component needs from the app root. The root has none today, so this renders the children. */
export function TestProviders({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
