import {
  QueryClient,
  dehydrate,
  queryOptions,
  type DehydratedState,
} from "@tanstack/react-query";
import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { categoriesQuery, BlogMissingError } from "@/data/lib/blog/options";
import { loadLocaleMessages, type LocaleMessages } from "../locale/catalogs";
import { resolveRequestLocale, type Locale } from "../locale/locales";
import type { BlogCategory } from "@/data/lib/blog/types";
import { blogPublicBase, blogNotFound, readBlog } from "./route-kit";

export { categoriesQuery };
export const messagesQuery = (locale: Locale) =>
  queryOptions({
    queryKey: ["blog", "messages", locale],
    queryFn: () => loadLocaleMessages(locale),
  });
export type BlogBase = {
  messages: LocaleMessages;
  categories: BlogCategory[];
  publicBase: string;
};
export type BlogDocument = { dehydratedState: DehydratedState };

export async function serverBase(request: Request, variant: string) {
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  const release = () => {
    request.signal.removeEventListener("abort", abort);
    client.clear();
  };
  const abort = () => {
    void client.cancelQueries();
  };
  request.signal.throwIfAborted();
  request.signal.addEventListener("abort", abort, { once: true });
  const [{ wrap }, messages, categories] = await Promise.all([
    pageLoader(request, { variant }),
    readBlog(() =>
      client.fetchQuery(messagesQuery(resolveRequestLocale(request))),
    ),
    readBlog(() => client.fetchQuery(categoriesQuery())),
  ]).catch((error) => {
    release();
    throw error;
  });
  const base: BlogBase = { messages, categories, publicBase: blogPublicBase() };
  const required = async <T>(load: () => Promise<T>): Promise<T> => {
    try {
      return await readBlog(load);
    } catch (error) {
      release();
      if (error instanceof BlogMissingError) blogNotFound(error.kind, base);
      throw error;
    }
  };
  return {
    client,
    base,
    required,
    wrap: <T>(payload: T) => {
      const state = dehydrate(client);
      release();
      return wrap({ ...payload, dehydratedState: state });
    },
  };
}
