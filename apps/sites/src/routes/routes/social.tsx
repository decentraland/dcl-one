import "@dcl/one-ui/foundations/foundations.css";
import { useCallback } from "react";
import { useSearchParams } from "react-router";

import { useAuth } from "@/data/lib/auth/context";
import { SocialPage } from "@/pages/social/SocialPage";

export const meta = () => [{ title: "Communities | Decentraland" }];

export default function SocialRoute() {
  const auth = useAuth();
  const [params, setParams] = useSearchParams();
  // Opening a community, or going back to the sections, leaves any post link behind.
  const onSelect = useCallback(
    (id: string | null) =>
      setParams((prev) => {
        const next = new URLSearchParams(prev);
        next.delete("post");
        if (id) next.set("community", id);
        else next.delete("community");
        return next;
      }),
    [setParams],
  );
  return (
    // Another account starts from its own reads and state.
    <SocialPage
      key={auth.address ?? "guest"}
      communityId={params.get("community")}
      postId={params.get("post")}
      onSelect={onSelect}
    />
  );
}
