import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useLoaderData, useNavigate, useSearchParams } from "react-router";

import {
  clearVerifiedWalletCookie,
  serializeVerifiedWalletCookie,
} from "@/core/lib/telemetry/session";
import { useAuth } from "@/data/lib/auth/context";
import { getWalletForToken } from "@/data/lib/auth/thirdweb/index";
import MagicCallback from "@/features/auth/login/MagicCallback";
import { safeRedirectTarget } from "@/features/auth/login/redirect";

import type { Route } from "./+types/auth.callback";

export function meta(_args: Route.MetaArgs) {
  return [{ title: "Signing you in \u{2014} Decentraland" }];
}

export function loader(_args: Route.LoaderArgs) {
  // Magic's publishable key: public by design, but deployment configuration, so it comes from the environment.
  return { magicApiKey: process.env.MAGIC_API_KEY || null };
}

export async function action({ request }: Route.ActionArgs) {
  if (request.method !== "POST") {
    return Response.json({ ok: false }, { status: 405 });
  }

  let token = "";
  let signout = false;
  try {
    const body = (await request.json()) as { token?: unknown; signout?: unknown };
    if (typeof body?.token === "string") token = body.token;
    if (body?.signout === true) signout = true;
  } catch {
  }

  if (signout) {
    return Response.json(
      { ok: true },
      { headers: { "Set-Cookie": clearVerifiedWalletCookie() } },
    );
  }

  const address = token ? await getWalletForToken(token, request.signal) : null;
  if (!address) {
    return Response.json(
      { ok: false },
      { status: 401, headers: { "Set-Cookie": clearVerifiedWalletCookie() } },
    );
  }

  return Response.json(
    { ok: true, address },
    {
      status: 200,
      headers: { "Set-Cookie": serializeVerifiedWalletCookie(address) },
    },
  );
}

function extractToken(authResultRaw: string): string | null {
  try {
    const parsed = JSON.parse(authResultRaw) as {
      storedToken?: { cookieString?: string; jwtToken?: string };
      token?: string;
      cookieString?: string;
    };
    return (
      parsed.storedToken?.cookieString ??
      parsed.storedToken?.jwtToken ??
      parsed.token ??
      parsed.cookieString ??
      null
    );
  } catch {
    return null;
  }
}

/**
 * Two sign-ins return here: Magic's (Google, Apple, Discord, X), which carries its target in `state`, and the in-app
 * wallet's, whose links carry `next` (and `authResult` once it succeeded). A return with either of the in-app ones is
 * the in-app wallet's, so a cancelled one keeps its own message and page.
 */
export default function AuthCallbackRoute() {
  const { magicApiKey } = useLoaderData<typeof loader>();
  const [params] = useSearchParams();
  const inApp = params.has("authResult") || params.has("next");
  return inApp ? <InAppCallback /> : <MagicCallback magicApiKey={magicApiKey} />;
}

function InAppCallback() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [error, setError] = useState<string | null>(null);
  const ran = useRef(false);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;

    // Only a path on this site: a crafted `next` must not carry the new session away, nor read as another host.
    const next = safeRedirectTarget(params.get("next"), new URL(window.location.href));
    const authResultRaw = params.get("authResult");
    if (!authResultRaw) {
      setError("Sign-in didn't return a result. Please try again.");
      return;
    }
    const token = extractToken(authResultRaw);
    if (!token) {
      setError("Couldn't read the sign-in result. Please try again.");
      return;
    }

    void (async () => {
      const address = await getWalletForToken(token);
      if (!address) {
        setError("Your sign-in expired before we could finish. Please try again.");
        return;
      }
      const identity = await auth.completeInAppSession({
        token,
        walletAddress: address,
      });
      if (identity) {
        await fetch(window.location.pathname, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ token }),
        }).catch(() => null);
        navigate(next, { replace: true });
      } else {
        setError(auth.error ?? "Couldn't finish signing you in.");
      }
    })();
  }, [auth, navigate, params]);

  return (
    <main style={wrap}>
      {error ? (
        <>
          <h1 style={{ fontSize: 22, margin: 0 }}>Sign-in failed</h1>
          <p style={{ color: "rgba(232,232,240,0.7)" }}>{error}</p>
          <a href="/" style={link}>
            &#x2190; Back to Decentraland
          </a>
        </>
      ) : (
        <p style={{ color: "rgba(232,232,240,0.7)" }}>Finishing sign-in&#x2026;</p>
      )}
    </main>
  );
}

const wrap: CSSProperties = {
  maxWidth: 520,
  margin: "0 auto",
  padding: "80px 24px",
  color: "#e8e8f0",
  font: "15px/1.6 system-ui, sans-serif",
  textAlign: "center",
};

const link: CSSProperties = {
  color: "#ff2d55",
  textDecoration: "none",
  font: "600 14px/1 system-ui, sans-serif",
};
