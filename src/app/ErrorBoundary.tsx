import { Component, type ReactNode } from "react";
import { t } from "@/i18n";

/** Replaces a crashed view with a way to reload; saved data lives in the backend. */
export class ErrorBoundary extends Component<
  { children: ReactNode },
  { error: boolean }
> {
  state = { error: false };
  static getDerivedStateFromError() {
    return { error: true };
  }
  render() {
    return this.state.error ? (
      <div className="fatal-error">
        <h1>{t("app.crash.title")}</h1>
        <p>{t("app.crash.detail")}</p>
        <button onClick={() => location.reload()}>
          {t("app.crash.reload")}
        </button>
      </div>
    ) : (
      this.props.children
    );
  }
}
