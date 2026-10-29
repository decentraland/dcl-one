import { sidLoader } from "./session";

export type PageDefaults = {
  variant: string;
};

type PageSession = ReturnType<typeof sidLoader>;

export { sidLoader };

export async function pageLoader(
  request: Request,
  defaults: PageDefaults,
) {
  return { ...sidLoader(request), assignment: defaults };
}

export async function pageLoaderWith<T>(
  request: Request,
  defaults: PageDefaults,
  load: (session: PageSession) => Promise<T>,
) {
  const session = sidLoader(request);
  return { ...session, assignment: defaults, data: await load(session) };
}