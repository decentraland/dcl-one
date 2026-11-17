import type { RouterContextProvider } from "react-router";

import { sidLoader } from "./session";

export type PageDefaults = {
  variant: string;
};

type PageSession = ReturnType<typeof sidLoader>;

export { sidLoader };

/** `context` is the loader's router context, where the root middleware left the request's session. */
export async function pageLoader(request: Request, context: Readonly<RouterContextProvider>, defaults: PageDefaults) {
  return { ...sidLoader(request, context), assignment: defaults };
}

export async function pageLoaderWith<T>(
  request: Request,
  context: Readonly<RouterContextProvider>,
  defaults: PageDefaults,
  load: (session: PageSession) => Promise<T>,
) {
  const session = sidLoader(request, context);
  return { ...session, assignment: defaults, data: await load(session) };
}
