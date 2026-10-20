import { themeStyle, type Theme } from "@/theme/theme";

// A miniature window painted with the theme's own variables, independent of the active theme.
export function ThemePreview({ theme }: { theme: Theme }) {
  return (
    <span className="theme-preview" style={themeStyle(theme)} aria-hidden>
      <span className="theme-preview-sidebar">
        <span />
        <span />
        <span />
      </span>
      <span className="theme-preview-main">
        <span className="theme-preview-dots">
          <span style={{ background: "var(--accent)" }} />
          <span style={{ background: "var(--success)" }} />
          <span style={{ background: "var(--warning)" }} />
          <span style={{ background: "var(--danger)" }} />
        </span>
        <span className="theme-preview-panel">
          <span className="theme-preview-row" />
          <span className="theme-preview-row selected" />
          <span className="theme-preview-row" />
        </span>
      </span>
    </span>
  );
}
