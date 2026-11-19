import type { Middleware } from '@reduxjs/toolkit';

type Action = { type?: unknown; meta?: { arg?: { options?: { secret?: unknown } } } };

const HEADER_STYLE = 'color: gray; font-weight: lighter;';

/** Hides a World access password from a logged action, leaving the dispatched one untouched. */
export function redactSecret<T extends Action>(action: T): T {
  const arg = action.meta?.arg;
  if (arg?.options?.secret === undefined) return action;
  return {
    ...action,
    meta: { ...action.meta, arg: { ...arg, options: { ...arg.options, secret: '<redacted>' } } },
  };
}

function formatTime(time: Date): string {
  const pad = (value: number, length: number) => String(value).padStart(length, '0');
  return `${pad(time.getHours(), 2)}:${pad(time.getMinutes(), 2)}:${pad(time.getSeconds(), 2)}.${pad(time.getMilliseconds(), 3)}`;
}

/**
 * Prints each action to the console as a group: the state before it, the action, the error
 * of a reducer or of a later middleware if there is one, and the state after it.
 */
export const logger: Middleware = store => next => action => {
  const time = new Date();
  const prevState = store.getState();
  let result: unknown;
  let failure: { error: unknown } | undefined;
  try {
    result = next(action);
  } catch (error) {
    failure = { error };
  }
  const logged = redactSecret(action as Action);

  console.group(
    `%c action %c${String(logged.type)} %c@ ${formatTime(time)}`,
    HEADER_STYLE,
    'color: inherit;',
    HEADER_STYLE,
  );
  console.log('%c prev state', 'color: #9E9E9E; font-weight: bold', prevState);
  console.log('%c action    ', 'color: #03A9F4; font-weight: bold', logged);
  if (failure) console.log('%c error     ', 'color: #F20404; font-weight: bold;', failure.error);
  console.log('%c next state', 'color: #4CAF50; font-weight: bold', store.getState());
  console.groupEnd();

  if (failure) throw failure.error;
  return result;
};
