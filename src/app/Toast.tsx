import { AlertCircle, Check, X } from "lucide-react";
import { useT } from "@/i18n";
import type { Notice } from "./useNotices";

export function Toast({
  notice,
  onDismiss,
}: {
  notice: Notice;
  onDismiss: () => void;
}) {
  const t = useT();
  return (
    <div
      className={`toast ${notice.error ? "toast-error" : ""}`}
      role={notice.error ? "alert" : "status"}
    >
      {notice.error ? <AlertCircle size={15} /> : <Check size={15} />}
      <span>{notice.message}</span>
      <button aria-label={t("app.dismiss")} onClick={onDismiss}>
        <X size={13} />
      </button>
    </div>
  );
}
