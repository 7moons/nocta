import type { MetricsSummary } from "@/generated/contracts";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { OverviewCard } from "./parts";
import { percent } from "./overview";

const DURATIONS = ["check", "launch"] as const;

const seconds = (ms: number) => (ms / 1000).toFixed(1);

/** Success rates and timings since Nocta was installed. */
export function PerformanceCard({ metrics }: { metrics: MetricsSummary }) {
  const t = useT();
  const format = useFormat();
  const launches = percent(metrics.launchesSuccessTotal, metrics.launchesTotal);
  const timeouts = metrics.errors.checkTimeout + metrics.errors.verifyTimeout;
  return (
    <OverviewCard
      className="overview-performance"
      title={t("dashboard.metrics.title")}
      subtitle={t("dashboard.metrics.subtitle")}
    >
      <div className="overview-rates">
        <Rate
          label={t("dashboard.metrics.launchRate")}
          value={launches}
          detail={t("dashboard.metrics.ofTotal", {
            done: metrics.launchesSuccessTotal,
            total: metrics.launchesTotal,
          })}
        />
        <div className="overview-rate">
          <small>{t("dashboard.metrics.checks")}</small>
          <strong>{format.number(metrics.checksTotal)}</strong>
          <span>{t("dashboard.metrics.timeouts", { count: timeouts })}</span>
        </div>
      </div>
      <table className="overview-durations">
        <thead>
          <tr>
            <th />
            <th>{t("dashboard.metrics.averageHead")}</th>
            <th>p95</th>
            <th>{t("dashboard.metrics.samplesHead")}</th>
          </tr>
        </thead>
        <tbody>
          {DURATIONS.map((kind) => (
            <tr key={kind}>
              <th>{t(`dashboard.metrics.${kind}`)}</th>
              <td className="tabular">
                {metrics[kind].samples
                  ? t("activity.seconds", {
                      seconds: seconds(metrics[kind].averageMs),
                    })
                  : "—"}
              </td>
              <td className="tabular">
                {metrics[kind].samples
                  ? t("activity.seconds", {
                      seconds: seconds(metrics[kind].p95Ms),
                    })
                  : "—"}
              </td>
              <td className="tabular">
                {format.number(metrics[kind].samples)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </OverviewCard>
  );
}

function Rate({
  label,
  value,
  detail,
}: {
  label: string;
  value: number | null;
  detail: string;
}) {
  return (
    <div className="overview-rate">
      <small>{label}</small>
      <strong>{value === null ? "—" : `${value}%`}</strong>
      <span className="rate-bar">
        <span style={{ width: `${value ?? 0}%` }} />
      </span>
      <span>{detail}</span>
    </div>
  );
}
