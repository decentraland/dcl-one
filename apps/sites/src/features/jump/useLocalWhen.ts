import { useEffect, useState } from "react";

import { formatEventStart } from "@/data/lib/catalyst/jump/jump";
import type { JumpCardView } from "./view";

/** The server renders the start in UTC; after hydration it shows in the viewer's own time zone. */
export function useLocalWhen(when: JumpCardView["when"]): JumpCardView["when"] {
  const [local, setLocal] = useState<string | null>(null);
  const dateTime = when?.dateTime;
  useEffect(() => {
    setLocal(dateTime ? formatEventStart(dateTime) : null);
  }, [dateTime]);
  return when && local ? { ...when, label: local } : when;
}
