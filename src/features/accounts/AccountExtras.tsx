import { MousePointerClick, Plus, X } from "lucide-react";
import type { AccountField, ClickPoint } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";

/** Most fields an account can have (the core refuses more). */
export const MAX_FIELDS = 10;

/** The user's own named values for an account, like "Server: EU". */
export function FieldsEditor({
  fields,
  onChange,
}: {
  fields: AccountField[];
  onChange: (fields: AccountField[]) => void;
}) {
  const t = useT();
  const set = (index: number, patch: Partial<AccountField>) =>
    onChange(
      fields.map((field, at) =>
        at === index ? { ...field, ...patch } : field,
      ),
    );
  return (
    <div className="account-fields">
      <span className="field-label">{t("accounts.fields.title")}</span>
      {fields.map((field, index) => (
        <div className="account-field-row" key={index}>
          <input
            aria-label={t("accounts.fields.name", { number: index + 1 })}
            placeholder={t("accounts.fields.namePlaceholder")}
            maxLength={30}
            value={field.name}
            onChange={(event) => set(index, { name: event.target.value })}
          />
          <input
            aria-label={t("accounts.fields.value", { number: index + 1 })}
            placeholder={t("accounts.fields.valuePlaceholder")}
            maxLength={200}
            value={field.value}
            onChange={(event) => set(index, { value: event.target.value })}
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t("accounts.fields.remove", { number: index + 1 })}
            onClick={() => onChange(fields.filter((_, at) => at !== index))}
          >
            <X />
          </Button>
        </div>
      ))}
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={fields.length >= MAX_FIELDS}
        onClick={() => onChange([...fields, { name: "", value: "" }])}
      >
        <Plus />
        {t("accounts.fields.add")}
      </Button>
    </div>
  );
}

/** Where anti-AFK clicks in this account's client, as percentages of the window. */
export function ClickPointEditor({
  point,
  onChange,
}: {
  point: ClickPoint | null;
  onChange: (point: ClickPoint | null) => void;
}) {
  const t = useT();
  const percent = (value: string) =>
    Math.min(100, Math.max(0, Math.round(Number(value) || 0)));
  return (
    <div className="click-point">
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={point !== null}
          onChange={(event) =>
            onChange(
              event.target.checked ? { xPercent: 50, yPercent: 50 } : null,
            )
          }
        />
        <MousePointerClick size={13} />
        {t("accounts.details.afkPoint")}
      </label>
      {point && (
        <span className="click-point-inputs">
          <label>
            X
            <input
              type="number"
              min={0}
              max={100}
              aria-label={t("accounts.details.afkX")}
              value={point.xPercent}
              onChange={(event) =>
                onChange({ ...point, xPercent: percent(event.target.value) })
              }
            />
            %
          </label>
          <label>
            Y
            <input
              type="number"
              min={0}
              max={100}
              aria-label={t("accounts.details.afkY")}
              value={point.yPercent}
              onChange={(event) =>
                onChange({ ...point, yPercent: percent(event.target.value) })
              }
            />
            %
          </label>
        </span>
      )}
      <small className="field-hint">
        {t("accounts.details.afkPointDetail")}
      </small>
    </div>
  );
}
