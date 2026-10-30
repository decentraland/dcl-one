import { useCallback, useEffect, useState } from "react";

export type AccountListing<T> = {
  value: T | null;
  reload: () => void;
  update: (change: (value: T) => T) => void;
};

export function useAccountListing<T>(owner: string | null, load: () => Promise<T>): AccountListing<T> {
  const [state, setState] = useState<{ owner: string | null; value: T | null }>({ owner, value: null });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let retired = false;
    let latest = 0;
    const run = () => {
      const call = ++latest;
      void load().then((value) => {
        if (!retired && call === latest) setState({ owner, value });
      });
    };
    run();
    window.addEventListener("focus", run);
    document.addEventListener("visibilitychange", run);
    return () => {
      retired = true;
      window.removeEventListener("focus", run);
      document.removeEventListener("visibilitychange", run);
    };
  }, [owner, load, attempt]);

  const reload = useCallback(() => setAttempt((n) => n + 1), []);
  const update = useCallback(
    (change: (value: T) => T) => setState((s) => (s.owner === owner && s.value !== null ? { owner, value: change(s.value) } : s)),
    [owner],
  );
  return { value: state.owner === owner ? state.value : null, reload, update };
}
