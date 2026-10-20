import type { AccountTask } from "@/generated/contracts";

// Labels for task kinds and statuses live in the translations: `task.kind.*`, `task.status.*`.
export const taskActive = (task: AccountTask) =>
  !["succeeded", "failed", "cancelled"].includes(task.status);
