import type { LucideIcon } from "lucide-react";
import { ChevronLeft } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigation } from "@/app/navigation";
import { useT } from "@/i18n";

// Shared header for top-level views: back button, icon, title, subtitle and actions.
export function ViewHeader({
  icon: Icon,
  title,
  subtitle,
  actions,
}: {
  icon: LucideIcon;
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
}) {
  const t = useT();
  const { back, canGoBack } = useNavigation();
  return (
    <header className="view-header">
      <div className="view-title">
        <button
          className="view-back"
          onClick={back}
          disabled={!canGoBack}
          aria-label={t("app.back")}
          title={t("app.backHint")}
        >
          <ChevronLeft size={16} />
        </button>
        <span className="view-icon">
          <Icon size={19} />
        </span>
        <div>
          <h1>{title}</h1>
          {subtitle && <p>{subtitle}</p>}
        </div>
      </div>
      {actions && <div className="header-actions">{actions}</div>}
    </header>
  );
}
