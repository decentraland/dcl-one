import type { Middleware } from '@reduxjs/toolkit';

import { misc } from '#preload';

import { deploy } from './slice';

/** Writes each publish's outcome to the app log, which a bug report attaches. */
export const deploymentLogMiddleware: Middleware = () => next => action => {
  const result = next(action);
  if (deploy.fulfilled.match(action)) {
    const { info } = action.payload;
    misc
      .logDeployment({
        outcome: 'published',
        title: info.title,
        rootCID: info.rootCID,
        target: info.isWorld ? 'world' : 'land',
        baseParcel: info.baseParcel,
      })
      .catch(() => undefined);
  } else if (deploy.rejected.match(action)) {
    const error = (action.payload ?? action.error) as { name?: string; message?: string };
    misc
      .logDeployment({
        outcome: 'failed',
        error: [error?.name, error?.message].filter(Boolean).join(': ') || 'unknown error',
      })
      .catch(() => undefined);
  }
  return result;
};
