// Probe (blog plan R3): what aborts a queryFn's signal in the installed @tanstack/react-query when a
// route's clientLoader awaits ensureInfiniteQueryData or fetchInfiniteQuery (which are not observers)
// while a component's InfiniteQueryObserver may be mounted. These assertions record measured
// behaviour; the blog's request-ownership design is built on them, not on assumptions.
import {
  InfiniteQueryObserver,
  QueryClient,
  type InfiniteData,
} from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

type Page = { items: number[]; total: number; skip: number; limit: number };
type Param = { skip: number; limit: number };

const tick = (ms = 20) => new Promise((resolve) => setTimeout(resolve, ms));

function setup() {
  const log = { calls: 0, aborted: [] as number[] };
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
  });
  const options = {
    queryKey: ["blog", "list", { scope: "home" }] as const,
    initialPageParam: { skip: 0, limit: 7 } as Param,
    getNextPageParam: () => undefined,
    queryFn: ({
      signal,
      pageParam,
    }: {
      signal: AbortSignal;
      pageParam: Param;
    }) => {
      const call = ++log.calls;
      return new Promise<Page>((resolve, reject) => {
        const timer = setTimeout(
          () => resolve({ items: [call], total: 1, ...pageParam }),
          200,
        );
        signal.addEventListener("abort", () => {
          log.aborted.push(call);
          clearTimeout(timer);
          reject(signal.reason);
        });
      });
    },
  };
  return { client, options, log };
}

const outcome = (promise: Promise<unknown>) =>
  promise.then(
    () => "resolved",
    (error: unknown) =>
      (error as { constructor?: { name?: string } })?.constructor?.name ??
      "rejected",
  );

const observe = (
  client: QueryClient,
  options: ReturnType<typeof setup>["options"],
) => {
  const observer = new InfiniteQueryObserver<
    Page,
    Error,
    InfiniteData<Page, Param>,
    typeof options.queryKey,
    Param
  >(client, options);
  return { observer, unsubscribe: observer.subscribe(() => undefined) };
};

describe.each(["ensureInfiniteQueryData", "fetchInfiniteQuery"] as const)(
  "when a clientLoader awaits %s",
  (via) => {
    it("should run one request to completion when nothing else is involved", async () => {
      const { client, options, log } = setup();

      expect(await outcome(client[via](options))).toBe("resolved");
      expect(log).toEqual({ calls: 1, aborted: [] });
    });

    it("should share one request with a mounted observer, and both get data", async () => {
      const { client, options, log } = setup();
      const { observer, unsubscribe } = observe(client, options);

      expect(await outcome(client[via](options))).toBe("resolved");
      await tick();
      expect(observer.getCurrentResult().data?.pages).toHaveLength(1);
      expect(log).toEqual({ calls: 1, aborted: [] });
      unsubscribe();
    });

    // The loader is not a consumer: unmounting the last observer aborts the shared request, and the
    // loader's own await rejects with CancelledError, leaving the query without data.
    it("should abort the shared request when the last observer unmounts, rejecting the loader too", async () => {
      const { client, options, log } = setup();
      const loading = outcome(client[via](options));
      const { unsubscribe } = observe(client, options);
      await tick();
      unsubscribe();

      expect(await loading).toBe("CancelledError");
      expect(log).toEqual({ calls: 1, aborted: [1] });
      expect(client.getQueryState(options.queryKey)?.status).toBe("pending");
    });

    it("should keep the shared request when another observer stays mounted", async () => {
      const { client, options, log } = setup();
      const loading = outcome(client[via](options));
      const first = observe(client, options);
      const second = observe(client, options);
      await tick();
      first.unsubscribe();

      expect(await loading).toBe("resolved");
      expect(log).toEqual({ calls: 1, aborted: [] });
      second.unsubscribe();
    });

    it("should start a fresh request when the loader retries after that cancellation", async () => {
      const { client, options, log } = setup();
      const loading = outcome(client[via](options));
      const { unsubscribe } = observe(client, options);
      await tick();
      unsubscribe();
      await loading;

      expect(await outcome(client[via](options))).toBe("resolved");
      expect(log).toEqual({ calls: 2, aborted: [1] });
    });

    it("should abort the request on cancelQueries", async () => {
      const { client, options, log } = setup();
      const loading = outcome(client[via](options));
      await tick();
      await client.cancelQueries({ queryKey: options.queryKey });

      expect(await loading).toBe("CancelledError");
      expect(log).toEqual({ calls: 1, aborted: [1] });
    });
  },
);

describe("when only an observer reads the query", () => {
  it("should abort the request when it unmounts mid-flight", async () => {
    const { client, options, log } = setup();
    const { unsubscribe } = observe(client, options);
    await tick();
    unsubscribe();
    await tick(250);

    expect(log).toEqual({ calls: 1, aborted: [1] });
  });
});

// Candidate ownership design: the clientLoader holds its own observer while it waits, so it counts as
// a consumer. Measured here before the design relies on it.
describe("when the clientLoader holds an observer while it waits", () => {
  const loaderAwait = (
    client: QueryClient,
    options: ReturnType<typeof setup>["options"],
    navigation: AbortSignal,
  ) => {
    const held = observe(client, options);
    return new Promise<string>((resolve) => {
      const done = (value: string) => {
        held.unsubscribe();
        resolve(value);
      };
      navigation.addEventListener("abort", () => done("navigation-aborted"));
      client.ensureInfiniteQueryData(options).then(
        () => done("resolved"),
        (error: unknown) => done((error as Error).constructor.name),
      );
    });
  };

  it("should keep the shared request when the component's observer unmounts", async () => {
    const { client, options, log } = setup();
    const navigation = new AbortController();
    const component = observe(client, options);
    const loading = loaderAwait(client, options, navigation.signal);
    await tick();
    component.unsubscribe();

    expect(await loading).toBe("resolved");
    expect(log).toEqual({ calls: 1, aborted: [] });
  });

  it("should abort the request when the navigation is abandoned and nothing else reads it", async () => {
    const { client, options, log } = setup();
    const navigation = new AbortController();
    const loading = loaderAwait(client, options, navigation.signal);
    await tick();
    navigation.abort();

    expect(await loading).toBe("navigation-aborted");
    await tick();
    expect(log).toEqual({ calls: 1, aborted: [1] });
  });

  it("should leave the request to a mounted component when the navigation is abandoned", async () => {
    const { client, options, log } = setup();
    const navigation = new AbortController();
    const component = observe(client, options);
    const loading = loaderAwait(client, options, navigation.signal);
    await tick();
    navigation.abort();
    await loading;
    await tick(250);

    expect(log).toEqual({ calls: 1, aborted: [] });
    expect(component.observer.getCurrentResult().data?.pages).toHaveLength(1);
    component.unsubscribe();
  });
});
