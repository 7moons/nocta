import type { CSSProperties, ReactNode } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";

export type Tone = "active" | "success" | "warning" | "danger" | "muted";

/**
 * One row of the tasks or instances tab. Every row has the same columns, so tasks, Nocta's
 * clients and outside clients line up: status, name, what, detail, meta and an action.
 */
export function ActivityLine({
  status,
  tone,
  name,
  what,
  whatTitle,
  detail,
  meta,
  action,
  style,
}: {
  status: string;
  tone: Tone;
  name: string;
  what?: string;
  whatTitle?: string;
  detail?: string;
  meta?: ReactNode;
  action?: { label: string; title?: string; run: () => void };
  style?: CSSProperties;
}) {
  return (
    <div role="listitem" className="activity-line" style={style}>
      <span className={`activity-pill tone-${tone}`}>{status}</span>
      <strong title={name}>{name}</strong>
      <span className="activity-what" title={whatTitle ?? what}>
        {what}
      </span>
      <span className="activity-detail" title={detail}>
        {detail}
      </span>
      <span className="activity-meta tabular">{meta}</span>
      <span className="activity-action">
        {action && (
          <Button
            variant="ghost"
            size="icon"
            aria-label={action.label}
            title={action.title ?? action.label}
            onClick={action.run}
          >
            <X />
          </Button>
        )}
      </span>
    </div>
  );
}
