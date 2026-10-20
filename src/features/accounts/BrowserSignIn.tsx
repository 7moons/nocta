import { useState } from "react";
import { Globe, LoaderCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

/**
 * Adds an account by signing in on Roblox's own page in a browser, so captchas, two-step
 * verification and passkeys work as usual and the password never passes through Nocta.
 */
export function BrowserSignIn({
  folderId,
  busy,
  onBusy,
  onDone,
}: {
  folderId: number | null;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onDone: () => void;
}) {
  const t = useT();
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState("");
  return (
    <div className="browser-sign-in">
      <Globe size={16} />
      <div>
        <strong>{t("import.signIn.title")}</strong>
        <small>
          {waiting ? t("import.signIn.waiting") : t("import.signIn.detail")}
        </small>
        {error && (
          <small className="form-error" role="alert">
            {error}
          </small>
        )}
      </div>
      <Button
        variant="outline"
        disabled={busy}
        onClick={async () => {
          setError("");
          setWaiting(true);
          onBusy(true);
          try {
            await api.signInAccount(folderId);
            onDone();
          } catch (failure) {
            setError(errorMessage(failure));
          } finally {
            setWaiting(false);
            onBusy(false);
          }
        }}
      >
        {waiting && <LoaderCircle className="spin" />}
        {t(waiting ? "import.signIn.busy" : "import.signIn.button")}
      </Button>
    </div>
  );
}
