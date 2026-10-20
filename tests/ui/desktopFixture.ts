import type { Page } from "@playwright/test";
import type { AppSnapshot, FriendEntry } from "../../src/generated/contracts";

declare global {
  interface Window {
    fixtureCalls: { command: string; args: Record<string, unknown> }[];
  }
}

export interface FixtureData {
  friends?: FriendEntry[];
  avatars?: Record<string, string>;
}

export const icon = (color: string) =>
  `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32'%3E%3Crect width='32' height='32' fill='%23${color}'/%3E%3C/svg%3E`;

/**
 * Loads the app with a fake desktop backend: the preview snapshot (with `count` accounts),
 * changed by `prepare`, and commands that update it like the real backend would. Every call
 * is recorded in `window.fixtureCalls`.
 */
export async function mockDesktop(
  page: Page,
  prepare: (snapshot: AppSnapshot) => void = () => {},
  data: FixtureData = {},
  count = 3,
) {
  await page.goto(`/?benchmark=${count}`);
  const snapshot: AppSnapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.platform = "windows";
  snapshot.runtime.capabilities = ["launch", "sessions"];
  snapshot.launcher.handlerRegistered = true;
  // Like the desktop app by default: drawn without the GPU.
  snapshot.runtime.safeDisplay = true;
  prepare(snapshot);
  await page.addInitScript(
    ({ initial, data }) => {
      const state = structuredClone(initial);
      const callbacks = new Map();
      const listeners = new Set<number>();
      let next = 0;
      const publish = () => {
        state.revision++;
        for (const id of listeners)
          callbacks.get(id)?.({
            event: "nocta:snapshot",
            id,
            payload: structuredClone(state),
          });
      };
      const account = (id: unknown) => {
        const found = state.accounts.find(
          (item: { id: number }) => item.id === id,
        );
        if (!found) throw new Error("Account not found.");
        return found;
      };
      Object.assign(window, { isTauri: true, fixtureCalls: [] });
      // Sends a snapshot event as the backend would, possibly without some parts.
      Object.assign(window, {
        fixtureEmit: (patch: Record<string, unknown>, omit: string[] = []) => {
          // Objects are merged (a config patch), everything else replaced.
          for (const [key, value] of Object.entries(patch)) {
            const current = state[key];
            if (
              current &&
              typeof current === "object" &&
              !Array.isArray(current)
            )
              Object.assign(current, value);
            else state[key] = value;
          }
          state.revision++;
          const payload = structuredClone(state);
          for (const key of omit) delete payload[key];
          for (const id of listeners)
            callbacks.get(id)?.({ event: "nocta:snapshot", id, payload });
        },
      });
      Object.assign(window, {
        __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
        __TAURI_INTERNALS__: {
          metadata: {
            currentWindow: { label: "main" },
            currentWebview: { label: "main" },
          },
          transformCallback(callback: unknown) {
            const id = ++next;
            callbacks.set(id, callback);
            return id;
          },
          unregisterCallback(id: number) {
            callbacks.delete(id);
          },
          async invoke(command: string, args: Record<string, any> = {}) {
            window.fixtureCalls.push({ command, args });
            switch (command) {
              case "plugin:event|listen":
                listeners.add(args.handler);
                return args.handler;
              case "plugin:event|unlisten":
                return;
              case "get_snapshot":
                return structuredClone(state);
              case "patch_config":
                Object.assign(state.config, args.patch);
                publish();
                return;
              case "get_place_info":
                return {
                  placeId: args.placeId,
                  name: `Game ${args.placeId}`,
                  universeId: "1",
                  iconUrl: null,
                };
              case "find_user":
                if (args.username === "nobody")
                  throw "No Roblox account is called nobody.";
                return {
                  id: "77",
                  name: args.username,
                  displayName: "Builder",
                };
              case "launch_game":
              case "run_preset":
                return [1];
              case "update_account_details":
                Object.assign(account(args.id), args.details);
                publish();
                return;
              case "set_totp":
                account(args.id).hasTotp = args.key !== null;
                publish();
                return;
              case "totp_code":
                return { code: "492039", remainingSecs: 21 };
              case "export_accounts":
                return true;
              case "sign_in_account": {
                const id = state.accounts.length + 1;
                state.accounts.push({
                  ...structuredClone(state.accounts[0]),
                  id,
                  username: "SignedInFriend",
                  userId: "9001",
                  hasCookie: true,
                  pinned: false,
                  alias: "",
                });
                publish();
                return id;
              }
              case "refresh_presence":
                for (const item of state.accounts)
                  item.presence = {
                    userPresenceType: item.id === 1 ? 2 : 1,
                    lastLocation: item.id === 1 ? "Doors" : "Website",
                    lastOnline: null,
                    placeId: item.id === 1 ? "91" : null,
                  };
                publish();
                return state.accounts.length;
              case "user_avatars":
                return Object.fromEntries(
                  (args.userIds as string[])
                    .filter((id) => data.avatars?.[id])
                    .map((id) => [id, data.avatars![id]]),
                );
              case "friends_of":
                return data.friends ?? [];
              case "test_webhook":
                if (!String(args.url).includes("discord.com/api/webhooks/"))
                  throw "Paste a Discord webhook link (https://discord.com/api/webhooks/…).";
                return;
              case "refresh_robux":
                for (const id of args.ids as number[]) {
                  account(id).robux = id * 1000;
                  account(id).robuxCheckedAt = new Date().toISOString();
                }
                publish();
                return (args.ids as number[]).length;
              case "allowed_fast_flags":
                return [
                  { name: "FIntDebugForceMSAASamples", kind: "int" },
                  { name: "DFFlagDisableDPIScale", kind: "bool" },
                  { name: "FFlagDebugSkyGray", kind: "bool" },
                ];
              case "start_session_tasks":
                return args.ids;
              case "copy_accounts":
                return "user:password:cookie";
              case "list_servers":
                return {
                  placeId: args.placeId,
                  warnings: [],
                  cancelled: false,
                  nextCursor: null,
                  items: [],
                };
              case "cancel_server_list":
                return;
              case "system_memory":
                // Like the user's computer: 31 GB of RAM, 87% of the commit limit reserved.
                return {
                  ramTotalMb: 31846,
                  ramFreeMb: 7475,
                  commitLimitMb: 49152,
                  commitUsedMb: 42700,
                };
              case "close_instance":
                state.instances = state.instances.filter(
                  (item: { id: string }) => item.id !== args.id,
                );
                publish();
                return;
              case "ui_ready":
              case "set_safe_display":
              case "sign_out_other_sessions":
              case "set_display_name":
              case "quick_login_approve":
                return;
              case "reorder_accounts":
                for (const item of state.accounts)
                  item.sortOrder = (args.ids as number[]).indexOf(item.id);
                for (const item of state.accounts)
                  if (item.sortOrder === -1) item.sortOrder = null;
                publish();
                return;
              case "privacy_settings":
                return [
                  {
                    key: "whoCanChatWithMeInExperiences",
                    value: "AllUsers",
                    options: ["AllUsers", "NoOne"],
                    restricted: [
                      { value: "AllUsers", requirement: "FacialAgeEstimation" },
                    ],
                  },
                  {
                    key: "whoCanJoinMeInExperiences",
                    value: "Friends",
                    options: ["AllUsers", "Friends", "NoOne"],
                    restricted: [],
                  },
                  {
                    key: "whoCanSeeMyInventory",
                    value: "NoOne",
                    options: ["AllUsers", "NoOne"],
                    restricted: [],
                  },
                ];
              case "set_privacy":
                return (args.ids as number[]).length;
              case "quick_login_check":
                if (args.code !== "AB3D9Z")
                  throw "Roblox did not recognise this code.";
                return {
                  device: "Chrome on Windows",
                  location: "Warsaw, Poland",
                };
              case "befriend_accounts":
                return {
                  pairs: 3,
                  added: 2,
                  already: 1,
                  pending: 0,
                  failed: 0,
                };
              case "save_folder": {
                if (args.id === null)
                  state.folders.push({
                    ...args.input,
                    id:
                      Math.max(
                        0,
                        ...state.folders.map((f: { id: number }) => f.id),
                      ) + 1,
                    sortOrder: state.folders.length,
                  });
                else
                  Object.assign(
                    state.folders.find((f: { id: number }) => f.id === args.id),
                    args.input,
                  );
                publish();
                return;
              }
              case "move_folder": {
                type F = {
                  id: number;
                  group: string | null;
                  sortOrder: number;
                };
                const moving = state.folders.find((f: F) => f.id === args.id);
                moving.group = args.group;
                const siblings = state.folders
                  .filter((f: F) => f.id !== args.id && f.group === args.group)
                  .sort((a: F, b: F) => a.sortOrder - b.sortOrder);
                const index = siblings.findIndex(
                  (f: F) => f.id === args.before,
                );
                siblings.splice(index < 0 ? siblings.length : index, 0, moving);
                siblings.forEach(
                  (f: F, order: number) => (f.sortOrder = order),
                );
                publish();
                return;
              }
              case "rename_folder_group":
                for (const folder of state.folders)
                  if (folder.group === args.from) folder.group = args.to;
                publish();
                return;
              case "delete_folder":
                state.folders = state.folders.filter(
                  (f: { id: number }) => f.id !== args.id,
                );
                for (const item of state.accounts)
                  if (item.folderId === args.id) item.folderId = null;
                publish();
                return;
              case "account_action":
                if (args.action === "move")
                  for (const id of args.ids as number[])
                    account(id).folderId = args.folderId;
                publish();
                return;
              case "arrange_windows":
                return state.instances.filter(
                  (item: { status: string }) => item.status === "running",
                ).length;
            }
            throw new Error(`Unexpected fixture command: ${command}`);
          },
        },
      });
    },
    { initial: snapshot, data },
  );
  await page.reload();
  return snapshot;
}

export const calls = (page: Page, command: string) =>
  page.evaluate(
    (name) => window.fixtureCalls.filter((call) => call.command === name),
    command,
  );

export const lastArgs = async (page: Page, command: string) =>
  (await calls(page, command)).at(-1)?.args;
