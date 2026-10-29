import ScreenFreshness from "@/features/components/chrome/ScreenFreshness";

import { useEffect, useRef } from "react";
import { data, redirect, useNavigate, useRevalidator, useSearchParams } from "react-router";

import CreatorHubHome, { type ChTemplateCard } from "@ui/creatorhub/pages/CreatorHubHome";
import { STARTER_TEMPLATES } from "@ui/creatorhub/pages/ChTemplates";

import { useAuth } from "@/data/lib/auth/index";
import { isDesktopShell } from "@/data/lib/auth/native-shell";
import { openSignIn } from "@/features/components/auth/signin-store";
import type { CreatorScene } from "@/data/lib/catalyst/create/index.server";
import { loadCreateScreen } from "@/data/lib/screens/create.server";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

import type { Route } from "./+types/create._index";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const meta = () => creatorHubMeta("Creator Hub");

const STORY: StoryId = "creator-hub/home";
const FALLBACK = { variant: "base" };

/** Where `/create?projectUrl=…` (the link dcl-scene-cli start prints) goes: the SDK workspace of the
    scene editor. The run token rides the link as a `#token=` fragment, which the browser carries
    across this redirect untouched and the editor route claims client-side; it never reaches a
    query string or this server. */
export function sdkWorkspaceHandoff(url: URL): string | null {
  const projectUrl = url.searchParams.get("projectUrl")?.trim();
  if (!projectUrl) return null;
  const target = new URLSearchParams({ source: "sdk", projectUrl, from: "home" });
  return `/create/scene-editor?${target.toString()}`;
}

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const handoff = sdkWorkspaceHandoff(url);
  if (handoff) throw redirect(handoff, { headers: { "Cache-Control": "private, no-store" } });

  const { sid, wrap } = await pageLoader(request, FALLBACK);
  const screen = await loadCreateScreen(request);
  const response = wrap({ sid, ...screen.data });
  const headers = new Headers(response.init?.headers);
  headers.set("Server-Timing", screen.serverTiming);
  headers.set("Cache-Control", "private, no-store");
  return data(response.data, { ...response.init, headers });
}

export function headers({ loaderHeaders, parentHeaders }: Route.HeadersArgs) {
  const headers = new Headers(parentHeaders);
  for (const name of ["Cache-Control", "Server-Timing"]) {
    const value = loaderHeaders.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
}

export default function CreateHome({ loaderData }: Route.ComponentProps) {
  const d = loaderData;

  return (
    <>
    <ScreenFreshness freshness={d.freshness} />
    <HubHomeView
      sid={d.sid}
      creator={d.creator}
      profileAddress={d.profileAddress}
      profileName={d.profileName}
      scenes={d.scenes}
      scenesError={d.scenesError}
    />
    </>
  );
}

const TEMPLATE_CARDS: ChTemplateCard[] = STARTER_TEMPLATES.map((t) => ({
  id: t.id,
  title: t.title,
  thumb: t.thumb,
  difficulty: t.difficulty_level,
  href: `/create/scene-editor?new=1&template=${encodeURIComponent(t.id)}&name=${encodeURIComponent(t.title)}&from=home`,
}));

function HubHomeView({
  sid,
  creator,
  profileAddress,
  profileName,
  scenes,
  scenesError,
}: {
  sid: string;
  creator: string;
  profileAddress: string;
  profileName: string;
  scenes: CreatorScene[];
  scenesError: boolean;
}) {
  const navigate = useNavigate();
  const revalidator = useRevalidator();
  const [, setSearchParams] = useSearchParams();
  const { isConnected, address } = useAuth();

  const name = isConnected && address?.toLowerCase() === profileAddress ? profileName : "";

  useEffect(() => {
    if (!isDesktopShell()) return;
    type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const tauri = (window as { __TAURI__?: { core: { invoke: Invoke } } }).__TAURI__;
    if (!tauri) {
      window.location.assign("/create?mp_ipc_error=no-global-tauri");
      return;
    }
    tauri.core
      .invoke("mp_hello", { name: "create-home" })
      .then((r) => tauri.core.invoke("mp_log", { msg: `round-trip ok: ${String(r)}` }))
      .catch((e: unknown) => {
        window.location.assign(`/create?mp_ipc_error=${encodeURIComponent(String(e))}`);
      });
  }, []);

  useEffect(() => {
    if (isConnected && address && (!creator || address.toLowerCase() !== profileAddress)) {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (!creator) next.set("creator", address);
          next.set("viewer", address);
          return next;
        },
        { replace: true, preventScrollReset: true },
      );
    }
  }, [isConnected, address, creator, profileAddress, setSearchParams]);

  const fired = useRef(false);
  useEffect(() => {
    if (fired.current) return;
    fired.current = true;

    track("ch_home_viewed", { scene_count: scenes.length }, { sid, story: STORY });

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sid]);

  const rescoping = isConnected && !!address && !creator;

  const sceneRows = scenes.map((s) => ({
    id: s.id,
    title: s.title,
    href: s.base_position
      ? `/create/scene-editor?pointer=${encodeURIComponent(s.base_position)}${s.world_name ? `&world=${encodeURIComponent(s.world_name)}` : ""}&from=home`
      :
        `/create/scene-editor?new=1&name=${encodeURIComponent(s.title)}&from=home`,
  }));

  function onScenesRetry() {
    track("ch_home_card_opened", { card: "scenes_retry" }, { sid, story: STORY });
    if (typeof revalidator.revalidate === "function") {
      revalidator.revalidate();
    } else {
      navigate(0);
    }
  }

  return (
      <CreatorHubHome
        scenes={sceneRows}
        scenesError={scenesError}
        rescoping={rescoping}
        templates={TEMPLATE_CARDS}
        onCardClick={(card) => {
          track("ch_home_card_opened", { card }, { sid, story: STORY });
        }}
        onRetry={onScenesRetry}
        signedIn={isConnected}
        account={address ?? ""}
        name={name}
        onSignIn={() => {
          track("ch_home_signin_clicked", {}, { sid, story: STORY });
          openSignIn();
        }}
        onStartBuilding={() => {
          track("ch_home_start_building", {}, { sid, story: STORY });
          navigate(sceneRows.length > 0 ? "/create/scenes" : "/create/templates");
        }}
        onScenes={() => {
          track("ch_scenes_clicked", { from: "home", card: "scenes" }, { sid, story: STORY });
        }}
        onLearn={() => {
          track("ch_home_card_opened", { card: "learn" }, { sid, story: STORY });
        }}
      />
  );
}
