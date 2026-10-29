import { useCallback } from "react";
import { useSearchParams } from "react-router";
import SocialPage from "@ui/social/SocialPage";
import { useAuth } from "@/data/lib/auth/context";
import { catalystBase } from "@/data/lib/catalyst/client";
import { openSignIn } from "@/features/components/auth/signin-store";

export const meta = () => [{ title: "Friends and communities | Decentraland" }];

export default function SocialRoute() {
  const auth = useAuth();
  const [params, setParams] = useSearchParams();
  const request = useCallback((path: string, init?: RequestInit) => {
    const url = `${catalystBase()}${path}`;
    return auth.identity ? auth.fetch(url, init) : fetch(url, init);
  }, [auth.identity, auth.fetch]);
  return <SocialPage key={auth.address ?? "guest"} viewer={auth.address} communityId={params.get("community")} postId={params.get("post")}
    request={request} onSignIn={() => openSignIn()}
    onSelect={id => setParams(prev => { const next = new URLSearchParams(prev); next.delete("post"); if (id) next.set("community", id); else next.delete("community"); return next; })} />;
}
