import { CheckCircle2, CircleAlert, CircleDashed } from "lucide-react";
import type { AppSnapshot } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT, type MessageKey } from "@/i18n";
import { OverviewCard } from "./parts";

type State = "done" | "warn" | "todo";

export interface SetupActions {
  onAdd: () => void;
  onAccounts: () => void;
  onSettings: (category: "client" | "launch") => void;
}

interface Step {
  id: string;
  state: State;
  title: MessageKey;
  detail: string;
  action?: { label: MessageKey; run: () => void };
}

/** What still has to be set up before accounts can be checked, solved and launched. */
export function SetupCard({
  snapshot,
  actions,
}: {
  snapshot: AppSnapshot;
  actions: SetupActions;
}) {
  const t = useT();
  const { config, runtime, launcher, client } = snapshot;
  const accounts = snapshot.accounts.filter((account) => !account.deletedAt);
  const ownClient = config.managedClient;
  const steps: Step[] = [
    {
      id: "accounts",
      state: accounts.length ? "done" : "todo",
      title: "dashboard.setup.accounts",
      detail: accounts.length
        ? t("accounts.count", { count: accounts.length })
        : t("dashboard.setup.accountsTodo"),
      action: accounts.length
        ? undefined
        : { label: "accounts.add", run: actions.onAdd },
    },
    {
      id: "browser",
      state: runtime.browserReady ? "done" : "warn",
      title: "dashboard.setup.browser",
      detail: t(
        runtime.browserReady
          ? "dashboard.setup.browserDone"
          : "dashboard.setup.browserTodo",
      ),
    },
    {
      id: "roblox",
      state: (ownClient ? !!client.active : launcher.handlerRegistered)
        ? "done"
        : "todo",
      title: "dashboard.setup.roblox",
      detail: t(
        ownClient
          ? client.active
            ? "dashboard.setup.robloxOwn"
            : "dashboard.setup.robloxOwnMissing"
          : launcher.handlerRegistered
            ? "dashboard.setup.robloxSystem"
            : "dashboard.setup.robloxMissing",
      ),
      action: (ownClient ? client.active : launcher.handlerRegistered)
        ? undefined
        : {
            label: "dashboard.setup.openClient",
            run: () => actions.onSettings("client"),
          },
    },
    ...(launcher.multiInstanceSupported
      ? [
          {
            id: "multi",
            state: (!config.multiInstance
              ? "todo"
              : launcher.multiInstance === "acquired"
                ? "done"
                : "warn") as State,
            title: "launch.options.multi" as MessageKey,
            detail: t(
              !config.multiInstance
                ? "dashboard.setup.multiOff"
                : launcher.multiInstance === "acquired"
                  ? "dashboard.setup.multiOn"
                  : "dashboard.setup.multiBusy",
            ),
            action: config.multiInstance
              ? undefined
              : {
                  label: "dashboard.setup.openLaunch" as MessageKey,
                  run: () => actions.onSettings("launch"),
                },
          },
        ]
      : []),
    {
      id: "target",
      state: config.placeId || config.inviteUrl ? "done" : "todo",
      title: "dashboard.setup.target",
      detail: t(
        config.placeId || config.inviteUrl
          ? "dashboard.setup.targetDone"
          : "dashboard.setup.targetTodo",
      ),
      action:
        config.placeId || config.inviteUrl
          ? undefined
          : { label: "launch.target.none", run: actions.onAccounts },
    },
  ];
  const done = steps.filter((step) => step.state === "done").length;
  // Once everything is ready the card has nothing to say.
  if (done === steps.length) return null;
  return (
    <OverviewCard
      className="overview-setup"
      title={t("dashboard.setup.title")}
      subtitle={t("dashboard.setup.progress", {
        done,
        total: steps.length,
      })}
    >
      <ul className="overview-rows setup-steps">
        {steps.map((step) => (
          <li key={step.id} className={`setup-${step.state}`}>
            {step.state === "done" ? (
              <CheckCircle2 size={15} />
            ) : step.state === "warn" ? (
              <CircleAlert size={15} />
            ) : (
              <CircleDashed size={15} />
            )}
            <span>
              <strong>{t(step.title)}</strong>
              <small>{step.detail}</small>
            </span>
            {step.action && (
              <Button variant="outline" size="sm" onClick={step.action.run}>
                {t(step.action.label)}
              </Button>
            )}
          </li>
        ))}
      </ul>
    </OverviewCard>
  );
}
