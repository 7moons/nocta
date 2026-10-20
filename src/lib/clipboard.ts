/** How long copied credentials stay on the clipboard. */
export const CLEAR_AFTER_MS = 30_000;

let pending: ReturnType<typeof setTimeout> | undefined;

/**
 * Copies text that may hold credentials and clears the clipboard again after
 * `CLEAR_AFTER_MS`, unless something else was copied since. When the clipboard cannot be read
 * back it is cleared anyway: a lost clipboard is better than a leaked password.
 */
export async function copySecret(text: string, clearAfterMs = CLEAR_AFTER_MS) {
  await navigator.clipboard.writeText(text);
  clearTimeout(pending);
  pending = setTimeout(() => {
    void (async () => {
      try {
        if ((await navigator.clipboard.readText()) !== text) return;
      } catch {
        /* unreadable: clear below */
      }
      await navigator.clipboard.writeText("").catch(() => {});
    })();
  }, clearAfterMs);
}
