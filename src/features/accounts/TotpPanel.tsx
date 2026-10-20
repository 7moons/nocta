import { useEffect, useState } from "react";
import { Copy } from "lucide-react";
import type { TotpCode } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { copySecret } from "@/lib/clipboard";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

/** The account's current two-step verification code, refreshed when it expires. */
export function TotpPanel({ accountId }: { accountId: number }) {
  const t = useT();
  const [code, setCode] = useState<{ value: TotpCode; at: number } | null>(
    null,
  );
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [now, setNow] = useState(Date.now);

  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, []);
  const remaining = code
    ? Math.max(0, code.value.remainingSecs - Math.floor((now - code.at) / 1000))
    : 0;
  const expired = code === null || remaining === 0;
  useEffect(() => {
    if (!expired) return;
    let live = true;
    api
      .totpCode(accountId)
      .then((value) => live && setCode({ value, at: Date.now() }))
      .catch((failure) => live && setError(errorMessage(failure)));
    return () => {
      live = false;
    };
  }, [accountId, expired]);

  if (error)
    return (
      <p className="form-error" role="alert">
        {error}
      </p>
    );
  return (
    <div className="totp-box" aria-label={t("accounts.totp.label")}>
      <span className="totp-code tabular" aria-live="polite">
        {code
          ? `${code.value.code.slice(0, 3)} ${code.value.code.slice(3)}`
          : "··· ···"}
      </span>
      <span className="totp-remaining">
        {t("accounts.totp.remaining", { seconds: remaining })}
      </span>
      <Button
        variant="outline"
        size="sm"
        disabled={!code}
        onClick={() => {
          if (!code) return;
          void copySecret(code.value.code).then(() => setCopied(true));
        }}
      >
        <Copy />
        {t(copied ? "accounts.totp.copied" : "accounts.totp.copy")}
      </Button>
    </div>
  );
}
