import { useState, type FormEvent } from "react";
import type { LaunchPreset, Schedule } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useLanguage, useT } from "@/i18n";
import { WEEKDAYS, weekdayName } from "./describe";

/** Creates or edits a schedule that runs a preset on set days and times. */
export function ScheduleDialog({
  schedule,
  presets,
  onSave,
  onClose,
}: {
  /** `undefined` closes the dialog, `null` creates a new schedule. */
  schedule: Schedule | null | undefined;
  presets: LaunchPreset[];
  onSave: (schedule: Schedule) => void;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <Dialog
      open={schedule !== undefined}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t(
        schedule ? "automation.schedule.edit" : "automation.schedule.new",
      )}
      description={t("automation.schedule.detail")}
    >
      {schedule !== undefined && (
        <ScheduleForm
          key={schedule?.id ?? "new"}
          schedule={schedule}
          presets={presets}
          onSave={(value) => {
            onSave(value);
            onClose();
          }}
          onCancel={onClose}
        />
      )}
    </Dialog>
  );
}

function ScheduleForm({
  schedule,
  presets,
  onSave,
  onCancel,
}: {
  schedule: Schedule | null;
  presets: LaunchPreset[];
  onSave: (schedule: Schedule) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const language = useLanguage();
  const [presetId, setPresetId] = useState(
    schedule?.presetId ?? presets[0]?.id ?? "",
  );
  const [days, setDays] = useState<number[]>(schedule?.days ?? [0, 1, 2, 3, 4]);
  const [time, setTime] = useState(schedule?.time ?? "08:00");
  const [closeAfter, setCloseAfter] = useState(
    schedule?.closeAfterMinutes ?? 0,
  );
  const valid =
    !!presetId &&
    days.length > 0 &&
    /^\d\d:\d\d$/.test(time) &&
    closeAfter >= 0;

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!valid) return;
    onSave({
      id: schedule?.id ?? crypto.randomUUID(),
      presetId,
      days: [...days].sort(),
      time,
      closeAfterMinutes: closeAfter,
      enabled: schedule?.enabled ?? true,
    });
  }

  return (
    <form className="dialog-body schedule-form" onSubmit={submit}>
      <label className="field-label">
        {t("automation.schedule.preset")}
        <select
          value={presetId}
          onChange={(event) => setPresetId(event.target.value)}
        >
          {presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </select>
      </label>
      <fieldset>
        <legend>{t("automation.schedule.days")}</legend>
        <div className="weekday-picker">
          {WEEKDAYS.map((day) => (
            <button
              key={day}
              type="button"
              aria-pressed={days.includes(day)}
              className={days.includes(day) ? "active" : ""}
              onClick={() =>
                setDays(
                  days.includes(day)
                    ? days.filter((item) => item !== day)
                    : [...days, day],
                )
              }
            >
              {weekdayName(language, day)}
            </button>
          ))}
        </div>
      </fieldset>
      <div className="form-grid">
        <label className="field-label">
          {t("automation.schedule.time")}
          <input
            type="time"
            required
            value={time}
            onChange={(event) => setTime(event.target.value)}
          />
        </label>
        <label className="field-label">
          {t("automation.schedule.closeAfter")}
          <input
            type="number"
            min={0}
            max={10080}
            value={closeAfter}
            onChange={(event) =>
              setCloseAfter(Math.max(0, Number(event.target.value) || 0))
            }
          />
        </label>
      </div>
      <small className="field-hint">
        {t("automation.schedule.closeAfterHint")}
      </small>
      <div className="dialog-footer">
        <Button type="button" variant="outline" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" disabled={!valid}>
          {t("common.save")}
        </Button>
      </div>
    </form>
  );
}
