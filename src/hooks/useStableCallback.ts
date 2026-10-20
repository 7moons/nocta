import { useCallback, useLayoutEffect, useRef } from "react";

/**
 * A callback whose identity never changes but which always calls the latest `callback`.
 * For event handlers passed to memoized children, such as table columns.
 */
export function useStableCallback<Args extends unknown[], Result>(
  callback: (...args: Args) => Result,
) {
  const latest = useRef(callback);
  useLayoutEffect(() => {
    latest.current = callback;
  });
  return useCallback((...args: Args) => latest.current(...args), []);
}
