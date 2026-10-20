import { Folder as FolderIcon } from "lucide-react";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { HealthBar, OverviewCard } from "./parts";
import type { Overview } from "./overview";

const SHOWN = 6;

/** The largest folders with their session states; a row opens the folder. */
export function FoldersCard({
  data,
  onFolder,
}: {
  data: Overview;
  onFolder: (id: number | null) => void;
}) {
  const t = useT();
  const format = useFormat();
  const rows = [
    ...data.folders.slice(0, SHOWN).map((item) => ({
      id: item.folder.id as number | null,
      name: item.folder.name,
      color: item.folder.color,
      total: item.total,
      counts: item.counts,
    })),
    ...(data.unfiled.total > 0
      ? [
          {
            id: null,
            name: t("accounts.folders.general"),
            color: undefined,
            ...data.unfiled,
          },
        ]
      : []),
  ];
  const hidden = data.folders.length - SHOWN;
  return (
    <OverviewCard
      className="overview-folders"
      title={t("sidebar.folders")}
      subtitle={
        hidden > 0 ? t("dashboard.folders.more", { count: hidden }) : undefined
      }
    >
      {rows.length === 0 ? (
        <p className="overview-empty">{t("dashboard.folders.empty")}</p>
      ) : (
        <ul className="folder-health">
          {rows.map((row) => (
            <li key={row.id ?? "general"}>
              <button onClick={() => onFolder(row.id)}>
                <FolderIcon size={14} style={{ color: row.color }} />
                <span className="folder-health-name">{row.name}</span>
                <HealthBar counts={row.counts} total={row.total} />
                <strong>{format.number(row.total)}</strong>
              </button>
            </li>
          ))}
        </ul>
      )}
    </OverviewCard>
  );
}
