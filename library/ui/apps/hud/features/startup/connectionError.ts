// The sign-in screens' Connection Error: Retry closes it and leaves the screen as it was; Exit leaves,
// where the page can. `body` replaces the generic text when there is something more useful to say
// (a desktop sign-in's own reason).
import { showDialog } from '../../design'
import { canExit, exitApplication } from '../gate/exitApplication'

export const CONNECTION_ERROR = {
  title: 'Connection Error',
  body: "We couldn't connect to the server. Please check your internet connection and try again later."
}

export function showConnectionError(body: string = CONNECTION_ERROR.body, signal?: AbortSignal): void {
  void showDialog({
    title: CONNECTION_ERROR.title,
    body,
    actions: [...(canExit() ? [{ id: 'exit', label: 'Exit', variant: 'secondary' as const }] : []), { id: 'retry', label: 'Retry' }],
    actionsEqual: true,
    signal
  }).then((choice) => {
    if (choice === 'exit') exitApplication()
  })
}
