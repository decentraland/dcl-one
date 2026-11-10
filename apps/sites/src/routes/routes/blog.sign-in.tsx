// /blog/sign-in: the blog home with the sign-in modal open, returning to the request's validated
// redirectTo, else /blog (blog plan R4-5, R5-4). No second auth flow; never a self-loop.
import { useEffect } from "react";
import { useSearchParams } from "react-router";

import { blogSignInTarget } from "@/features/blog/sign-in";
import { openSignIn } from "@/features/components/auth/signin-store";

import { BlogErrorBoundary, blogHeaders } from "../lib/blog-page";
import {
  BlogHome,
  homeClientLoader,
  homeLoader,
  type HomeData,
} from "../lib/blog-home";
import type { Route } from "./+types/blog.sign-in";

export const loader = ({ request }: Route.LoaderArgs) =>
  homeLoader({ request });

export const clientLoader = homeClientLoader;

export const meta = () => [];

export default function BlogSignInRoute({ loaderData }: Route.ComponentProps) {
  const [params] = useSearchParams();
  const redirectTo = params.get("redirectTo");
  useEffect(() => {
    openSignIn({ redirectTo: blogSignInTarget(redirectTo) });
  }, [redirectTo]);
  return <BlogHome data={loaderData as HomeData} />;
}

export const ErrorBoundary = BlogErrorBoundary;

export const headers = blogHeaders;
