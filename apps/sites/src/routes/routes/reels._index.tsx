import { ReelsEmpty, ReelsPage } from "@dcl/one-ui/apps/sites/reels/ReelsPage";

import { REELS_TITLE } from "../lib/reels-route";

export function meta() {
  return [{ title: REELS_TITLE }];
}

export default function ReelsIndex() {
  return (
    <ReelsPage title={REELS_TITLE}>
      <ReelsEmpty reason="photo" />
    </ReelsPage>
  );
}
