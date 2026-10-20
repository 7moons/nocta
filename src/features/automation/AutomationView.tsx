import { useState } from "react";
import {
  CalendarClock,
  Clock,
  Moon,
  Pencil,
  Play,
  Plus,
  Trash2,
  Workflow,
} from "lucide-react";
import type {
  AppConfig,
  AppSnapshot,
  LaunchPreset,
  Schedule,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ViewHeader } from "@/components/ViewHeader";
import { useLanguage, useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import type { RunAction } from "@/lib/actions";
import { api } from "@/lib/ipc";
import { describeDays, describeTarget, nextRun } from "./describe";
import { PresetDialog } from "./PresetDialog";
import { ScheduleDialog } from "./ScheduleDialog";

/** Launch presets (accounts + where to launch them) and schedules that run them. */
export function AutomationView({
  snapshot,
  onConfig,
  run,
}: {
  snapshot: AppSnapshot;
  onConfig: (patch: Partial<AppConfig>) => void;
  run: RunAction;
}) {
  const t = useT();
  const format = useFormat();
  const language = useLanguage();
  const config = snapshot.config;
  const presets = config.launchPresets;
  const schedules = config.schedules;
  const accounts = snapshot.accounts.filter((account) => !account.deletedAt);
  const [editingPreset, setEditingPreset] = useState<
    LaunchPreset | null | undefined
  >(undefined);
  const [editingSchedule, setEditingSchedule] = useState<
    Schedule | null | undefined
  >(undefined);
  const canLaunch = snapshot.runtime.capabilities.includes("launch");

  const savePreset = (preset: LaunchPreset) =>
    onConfig({
      launchPresets: presets.some((item) => item.id === preset.id)
        ? presets.map((item) => (item.id === preset.id ? preset : item))
        : [...presets, preset],
    });
  // A preset's schedules go with it.
  const deletePreset = (id: string) =>
    onConfig({
      launchPresets: presets.filter((item) => item.id !== id),
      schedules: schedules.filter((item) => item.presetId !== id),
    });
  const saveSchedule = (schedule: Schedule) =>
    onConfig({
      schedules: schedules.some((item) => item.id === schedule.id)
        ? schedules.map((item) => (item.id === schedule.id ? schedule : item))
        : [...schedules, schedule],
    });

  return (
    <>
      <ViewHeader
        icon={Workflow}
        title={t("views.automation")}
        subtitle={t("automation.subtitle")}
        actions={
          <Button onClick={() => setEditingPreset(null)}>
            <Plus />
            {t("automation.preset.new")}
          </Button>
        }
      />
      <div className="standard-content automation-view">
        <section className="settings-section">
          <header>
            <h3>{t("automation.preset.title")}</h3>
            <p>{t("automation.preset.subtitle")}</p>
          </header>
          {presets.length === 0 ? (
            <p className="automation-empty">{t("automation.preset.empty")}</p>
          ) : (
            <ul
              className="automation-list"
              aria-label={t("automation.preset.title")}
            >
              {presets.map((preset) => {
                const existing = preset.accountIds.filter((id) =>
                  accounts.some((account) => account.id === id),
                ).length;
                return (
                  <li key={preset.id} className="automation-row">
                    <div className="automation-main">
                      <strong>{preset.name}</strong>
                      <small>
                        {t("automation.preset.accountCount", {
                          count: existing,
                        })}{" "}
                        · {describeTarget(t, preset.target)}
                        {preset.afk && (
                          <span className="badge automation-afk">
                            <Moon size={11} />
                            AFK
                          </span>
                        )}
                      </small>
                    </div>
                    <Button
                      size="sm"
                      disabled={!canLaunch || existing === 0}
                      aria-label={t("automation.preset.runName", {
                        name: preset.name,
                      })}
                      onClick={() =>
                        run(
                          () => api.runPreset(preset.id),
                          t("automation.preset.started", {
                            name: preset.name,
                          }),
                        )
                      }
                    >
                      <Play />
                      {t("automation.preset.run")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("automation.preset.editName", {
                        name: preset.name,
                      })}
                      onClick={() => setEditingPreset(preset)}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("automation.preset.deleteName", {
                        name: preset.name,
                      })}
                      onClick={() => deletePreset(preset.id)}
                    >
                      <Trash2 />
                    </Button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="settings-section">
          <header className="automation-section-header">
            <div>
              <h3>{t("automation.schedule.title")}</h3>
              <p>{t("automation.schedule.subtitle")}</p>
            </div>
            <Button
              variant="outline"
              size="sm"
              disabled={!presets.length}
              title={
                presets.length ? undefined : t("automation.schedule.needPreset")
              }
              onClick={() => setEditingSchedule(null)}
            >
              <CalendarClock />
              {t("automation.schedule.new")}
            </Button>
          </header>
          {schedules.length === 0 ? (
            <p className="automation-empty">{t("automation.schedule.empty")}</p>
          ) : (
            <ul
              className="automation-list"
              aria-label={t("automation.schedule.title")}
            >
              {schedules.map((schedule) => {
                const preset = presets.find(
                  (item) => item.id === schedule.presetId,
                );
                const next = nextRun(schedule);
                return (
                  <li key={schedule.id} className="automation-row">
                    <Clock size={15} className="automation-icon" />
                    <div className="automation-main">
                      <strong>
                        {format.clock(schedule.time)} · {preset?.name ?? "—"}
                      </strong>
                      <small>
                        {describeDays(t, language, schedule.days)}
                        {schedule.closeAfterMinutes > 0 &&
                          ` · ${t("automation.schedule.closesAfter", { minutes: schedule.closeAfterMinutes })}`}
                        {next &&
                          ` · ${t("automation.schedule.next", { time: format.dateTime(next.toISOString()) ?? "" })}`}
                      </small>
                    </div>
                    <Switch
                      label={t("automation.schedule.enabled")}
                      checked={schedule.enabled}
                      onCheckedChange={(enabled) =>
                        saveSchedule({ ...schedule, enabled })
                      }
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("automation.schedule.edit")}
                      onClick={() => setEditingSchedule(schedule)}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("automation.schedule.delete")}
                      onClick={() =>
                        onConfig({
                          schedules: schedules.filter(
                            (item) => item.id !== schedule.id,
                          ),
                        })
                      }
                    >
                      <Trash2 />
                    </Button>
                  </li>
                );
              })}
            </ul>
          )}
          <p className="muted automation-note">
            {t("automation.schedule.note")}
          </p>
        </section>
      </div>
      <PresetDialog
        preset={editingPreset}
        accounts={accounts}
        onSave={savePreset}
        onClose={() => setEditingPreset(undefined)}
      />
      <ScheduleDialog
        schedule={editingSchedule}
        presets={presets}
        onSave={saveSchedule}
        onClose={() => setEditingSchedule(undefined)}
      />
    </>
  );
}
