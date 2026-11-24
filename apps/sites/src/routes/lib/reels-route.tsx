import { isRouteErrorResponse, useRevalidator, useRouteError } from "react-router";

import { ReelsEmpty, ReelsPage } from "@dcl/one-ui";

export const REELS_TITLE = "Decentraland Reels";

export function ReelErrorBoundary({ missing = "photo" }: { missing?: "photo" | "invalid-gallery" }) {
  const error = useRouteError();
  const revalidator = useRevalidator();
  const notFound = isRouteErrorResponse(error) && error.status === 404;
  return (
    <ReelsPage title={REELS_TITLE}>
      <ReelsEmpty reason={notFound ? missing : "unavailable"} onRetry={notFound ? undefined : revalidator.revalidate} />
    </ReelsPage>
  );
}
