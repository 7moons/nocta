import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import type { AllowedFlag, FastFlag } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { api, desktop } from "@/lib/ipc";
import { SettingSection } from "../SettingControls";

/**
 * FastFlags for Nocta's own Roblox client. Roblox reads only the flags on its allowlist, so
 * only those can be picked; a value is True/False or a whole number depending on the flag.
 */
export function FastFlagsSection({
  flags,
  onChange,
}: {
  flags: FastFlag[];
  onChange: (flags: FastFlag[]) => void;
}) {
  const t = useT();
  const [allowed, setAllowed] = useState<AllowedFlag[]>([]);
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  useEffect(() => {
    if (desktop) void api.allowedFastFlags().then(setAllowed, () => {});
  }, []);
  const available = allowed.filter(
    (flag) => !flags.some((item) => item.name === flag.name),
  );
  const kind = allowed.find((flag) => flag.name === name)?.kind;
  const valid =
    !!name &&
    (kind === "bool"
      ? /^(true|false)$/i.test(value)
      : /^-?\d+$/.test(value.trim()));

  return (
    <SettingSection
      id="client-flags"
      title={t("client.flags.title")}
      description={t("client.flags.subtitle")}
    >
      <ul className="fast-flags" aria-label={t("client.flags.title")}>
        {flags.map((flag) => (
          <li key={flag.name}>
            <code>{flag.name}</code>
            <span className="fast-flag-value">{flag.value}</span>
            <Button
              variant="ghost"
              size="icon"
              aria-label={t("client.flags.remove", { name: flag.name })}
              onClick={() =>
                onChange(flags.filter((item) => item.name !== flag.name))
              }
            >
              <Trash2 />
            </Button>
          </li>
        ))}
        {flags.length === 0 && (
          <li className="fast-flags-empty">{t("client.flags.empty")}</li>
        )}
      </ul>
      <form
        className="fast-flag-add"
        onSubmit={(event) => {
          event.preventDefault();
          if (!valid) return;
          onChange([...flags, { name, value: value.trim() }]);
          setName("");
          setValue("");
        }}
      >
        <select
          aria-label={t("client.flags.flag")}
          value={name}
          onChange={(event) => {
            setName(event.target.value);
            const next = allowed.find(
              (flag) => flag.name === event.target.value,
            );
            setValue(next?.kind === "bool" ? "True" : "");
          }}
        >
          <option value="">{t("client.flags.choose")}</option>
          {available.map((flag) => (
            <option key={flag.name} value={flag.name}>
              {flag.name}
            </option>
          ))}
        </select>
        {kind === "bool" ? (
          <select
            aria-label={t("client.flags.value")}
            value={value}
            onChange={(event) => setValue(event.target.value)}
          >
            <option value="True">True</option>
            <option value="False">False</option>
          </select>
        ) : (
          <input
            aria-label={t("client.flags.value")}
            inputMode="numeric"
            placeholder={t("client.flags.number")}
            value={value}
            disabled={!name}
            onChange={(event) => setValue(event.target.value)}
          />
        )}
        <Button type="submit" variant="outline" disabled={!valid}>
          <Plus />
          {t("client.flags.add")}
        </Button>
      </form>
      <p className="client-note">{t("client.flags.note")}</p>
    </SettingSection>
  );
}
