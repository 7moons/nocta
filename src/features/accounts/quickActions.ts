import {
  Globe,
  KeyRound,
  Pencil,
  Play,
  RefreshCw,
  ShieldCheck,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { useMemo } from "react";
import { useT } from "@/i18n";
import { useUiPreference } from "@/hooks/useUiPreference";

// Per-row buttons in the account table, in row order. Labels: `quickActions.<id>`.
export const QUICK_ACTIONS = [
  { id: "launch", icon: Play, defaultOn: true },
  { id: "open_browser", icon: Globe, defaultOn: true },
  { id: "check_health", icon: ShieldCheck, defaultOn: true },
  { id: "validate_cookie", icon: KeyRound, defaultOn: false },
  { id: "refresh_cookie", icon: RefreshCw, defaultOn: false },
  { id: "edit", icon: Pencil, defaultOn: false },
  { id: "trash", icon: Trash2, defaultOn: true },
] as const satisfies readonly {
  id: string;
  icon: LucideIcon;
  defaultOn: boolean;
}[];

export type QuickActionId = (typeof QUICK_ACTIONS)[number]["id"];

const DEFAULT_QUICK_ACTIONS: string[] = QUICK_ACTIONS.filter(
  (action) => action.defaultOn,
).map((action) => action.id);

/** Enabled quick actions with their labels, stored with the other per-profile UI preferences. */
export function useQuickActions() {
  const t = useT();
  const [enabled, setEnabled] = useUiPreference<string[]>(
    "accountQuickActions",
    DEFAULT_QUICK_ACTIONS,
  );
  const actions = useMemo(
    () =>
      QUICK_ACTIONS.filter((action) => enabled.includes(action.id)).map(
        (action) => ({ ...action, label: t(`quickActions.${action.id}`) }),
      ),
    [enabled, t],
  );
  const toggle = (id: QuickActionId, on: boolean) =>
    setEnabled(
      on
        ? [...enabled.filter((item) => item !== id), id]
        : enabled.filter((item) => item !== id),
    );
  return { actions, enabled, toggle };
}
