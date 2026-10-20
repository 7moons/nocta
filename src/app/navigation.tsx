import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";

export type View =
  "accounts" | "dashboard" | "friends" | "automation" | "settings" | "trash";
export type FolderSelection = number | null | "all";
/** Session-state filter of the accounts view: "all" or an `AccountHealth`. */
export type HealthFilter = string;
export interface Location {
  view: View;
  folder: FolderSelection;
  health: HealthFilter;
}

const LIMIT = 50;

// In-app history like a browser's: navigate() drops any forward entries,
// back()/forward() move through what was visited.
export function useNavigationHistory(initial: Location) {
  const [history, setHistory] = useState({ entries: [initial], index: 0 });
  const current = history.entries[history.index];
  const navigate = useCallback((patch: Partial<Location>) => {
    setHistory(({ entries, index }) => {
      const next = { ...entries[index], ...patch };
      if (
        next.view === entries[index].view &&
        next.folder === entries[index].folder &&
        next.health === entries[index].health
      )
        return { entries, index };
      const kept = [...entries.slice(0, index + 1), next].slice(-LIMIT);
      return { entries: kept, index: kept.length - 1 };
    });
  }, []);
  // Corrects the current entry without adding history, e.g. when its folder was deleted.
  const replace = useCallback((patch: Partial<Location>) => {
    setHistory(({ entries, index }) => ({
      entries: entries.map((entry, position) =>
        position === index ? { ...entry, ...patch } : entry,
      ),
      index,
    }));
  }, []);
  const go = useCallback((delta: number) => {
    setHistory(({ entries, index }) => ({
      entries,
      index: Math.min(entries.length - 1, Math.max(0, index + delta)),
    }));
  }, []);
  return {
    current,
    navigate,
    replace,
    back: useCallback(() => go(-1), [go]),
    forward: useCallback(() => go(1), [go]),
    canGoBack: history.index > 0,
    canGoForward: history.index < history.entries.length - 1,
  };
}

interface NavigationContextValue {
  back: () => void;
  canGoBack: boolean;
}

const NavigationContext = createContext<NavigationContextValue>({
  back: () => {},
  canGoBack: false,
});

export function NavigationProvider({
  back,
  canGoBack,
  children,
}: NavigationContextValue & { children: React.ReactNode }) {
  const value = useMemo(() => ({ back, canGoBack }), [back, canGoBack]);
  return (
    <NavigationContext.Provider value={value}>
      {children}
    </NavigationContext.Provider>
  );
}

export function useNavigation() {
  return useContext(NavigationContext);
}
