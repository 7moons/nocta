import raw from "../../../CHANGELOG.md?raw";

export interface ChangelogSection {
  title: string;
  items: string[];
}

export interface Release {
  version: string;
  date: string;
  summary: string;
  sections: ChangelogSection[];
}

/** Releases from a Keep a Changelog file, newest first as the file lists them. */
export function parseChangelog(text: string): Release[] {
  const releases: Release[] = [];
  let section: ChangelogSection | null = null;
  for (const line of text.split(/\r?\n/)) {
    const heading = /^## \[([^\]]+)\](?: - (\S+))?/.exec(line);
    if (heading) {
      releases.push({
        version: heading[1],
        date: heading[2] ?? "",
        summary: "",
        sections: [],
      });
      section = null;
      continue;
    }
    const release = releases.at(-1);
    if (!release) continue;
    const title = /^### (.+)/.exec(line);
    if (title) {
      section = { title: title[1].trim(), items: [] };
      release.sections.push(section);
    } else if (line.startsWith("- ") && section) {
      section.items.push(line.slice(2).trim());
    } else if (line.trim() && !section && !line.startsWith("[")) {
      release.summary = `${release.summary} ${line.trim()}`.trim();
    }
  }
  return releases;
}

export const RELEASES = parseChangelog(raw);
