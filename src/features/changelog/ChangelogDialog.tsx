import { Fragment, useEffect, useState } from "react";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { RELEASES } from "./changelog";

const SEEN_KEY = "nocta.changelog.seen";

/** Text with `code` spans, as the changelog writes them. */
function Inline({ text }: { text: string }) {
  return (
    <>
      {text
        .split("`")
        .map((part, index) =>
          index % 2 ? (
            <code key={index}>{part}</code>
          ) : (
            <Fragment key={index}>{part}</Fragment>
          ),
        )}
    </>
  );
}

/**
 * Whether the changelog is open. It opens by itself once after Nocta was updated to `version`
 * (null until the real version is known); a first install only remembers the version.
 */
export function useChangelog(version: string | null) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!version) return;
    try {
      const seen = localStorage.getItem(SEEN_KEY);
      if (seen && seen !== version) setOpen(true);
      localStorage.setItem(SEEN_KEY, version);
    } catch {
      /* without storage it only opens when asked */
    }
  }, [version]);
  return { open, setOpen };
}

export function ChangelogDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const t = useT();
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("changelog.title")}
      description={t("changelog.subtitle")}
      wide
    >
      <div className="dialog-body changelog">
        {RELEASES.map((release) => (
          <article key={release.version} className="changelog-release">
            <header>
              <h3>v{release.version}</h3>
              {release.date && (
                <time dateTime={release.date}>{release.date}</time>
              )}
            </header>
            {release.summary && <p>{release.summary}</p>}
            {release.sections.map((section) => (
              <section key={section.title}>
                <h4>{section.title}</h4>
                <ul>
                  {section.items.map((item) => (
                    <li key={item}>
                      <Inline text={item} />
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </article>
        ))}
      </div>
    </Dialog>
  );
}
