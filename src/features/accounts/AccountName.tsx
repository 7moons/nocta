import { Pin } from "lucide-react";
import type { Account } from "@/generated/contracts";
import { useT } from "@/i18n";
import { useAvatar } from "@/lib/avatars";
import { usePrivacy } from "@/lib/privacy";
import { sessionStale, STALE_AFTER_DAYS } from "./accountDetails";

const PRESENCE_CLASS = ["offline", "online", "in-game", "in-studio"] as const;

/** Avatar with an online dot; empty-handed accounts show their initial instead. */
export function AccountAvatar({ account }: { account: Account }) {
  const privacy = usePrivacy();
  const url = useAvatar(privacy.hidden ? null : account.userId);
  const presence = account.presence
    ? PRESENCE_CLASS[account.presence.userPresenceType]
    : undefined;
  return (
    <span
      className="avatar"
      style={account.color ? { borderColor: account.color } : undefined}
    >
      {url ? (
        <img src={url} alt="" loading="lazy" draggable={false} />
      ) : (
        privacy.username(account).replace("#", "").slice(0, 1).toUpperCase()
      )}
      {presence && presence !== "offline" && (
        <span className={`presence-dot ${presence}`} />
      )}
    </span>
  );
}

/** The account cell: avatar, username or alias, and markers for pins, notes and old sessions. */
export function AccountName({
  account,
  title,
}: {
  account: Account;
  title?: string;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const stale = sessionStale(account);
  const realName = privacy.realName(account);
  return (
    <div className="account-name" title={title}>
      <AccountAvatar account={account} />
      <div>
        <strong>
          {account.pinned && (
            <Pin
              size={11}
              className="account-pin"
              aria-label={t("accounts.details.pinned")}
            />
          )}
          {privacy.name(account)}
          {realName && <span className="account-real-name">{realName}</span>}
        </strong>
        <small>
          #{account.id} ·{" "}
          {account.userId && !privacy.hidden
            ? t("accounts.table.userId", { id: account.userId })
            : t("accounts.table.robloxAccount")}
          {stale && (
            <span
              className="stale-flag"
              title={t("accounts.details.staleDetail", {
                days: STALE_AFTER_DAYS,
              })}
            >
              {t("accounts.details.stale")}
            </span>
          )}
          {account.note && (
            <span className="account-note" title={account.note}>
              {account.note}
            </span>
          )}
        </small>
      </div>
    </div>
  );
}
