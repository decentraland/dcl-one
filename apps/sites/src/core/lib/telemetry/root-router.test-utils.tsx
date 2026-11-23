import { createContext, useContext, useState, type ReactNode } from "react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";

/** The root loader's session, as `root.tsx` returns it. */
export type RootSessionData = { sid?: string | null; analyticsUserId?: string | null };

const Rendered = createContext<ReactNode>(null);

function Slot() {
  return <>{useContext(Rendered)}</>;
}

/**
 * A drop-in for `MemoryRouter` in tests of components that read the root loader's session (the site chrome's
 * clicks, page views): a data router whose root route (id `root`) carries `root` as its loader data, as every
 * production document has. `children` render at every path and follow re-renders.
 */
export function RootDataRouter({
  initialEntries = ["/"],
  root = {},
  children,
}: {
  initialEntries?: string[];
  root?: RootSessionData;
  children?: ReactNode;
}) {
  const [router] = useState(() =>
    createMemoryRouter(
      [{ id: "root", loader: () => root, Component: Outlet, children: [{ path: "*", Component: Slot }] }],
      { initialEntries, hydrationData: { loaderData: { root } } },
    ),
  );
  return (
    <Rendered value={children}>
      <RouterProvider router={router} />
    </Rendered>
  );
}
