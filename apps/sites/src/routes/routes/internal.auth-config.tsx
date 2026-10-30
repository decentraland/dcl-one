/** Public sign-in configuration, also consumed by the static /play document. */
export function loader() {
  return Response.json({ walletConnectProjectId: process.env.WALLETCONNECT_PROJECT_ID || "" }, {
    headers: { "cache-control": "no-store" },
  });
}
