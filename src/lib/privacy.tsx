import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { Account } from "@/generated/contracts";
import { useT } from "@/i18n";

type Named = Pick<Account, "id" | "alias" | "username">;

export interface Privacy {
  /** Streamer mode: usernames are hidden, aliases stay. */
  hidden: boolean;
  /** What to call the account: its alias, else its username (or number when hidden). */
  name: (account: Named) => string;
  /** The username itself; in streamer mode the alias or account number instead. */
  username: (account: Named) => string;
  /** The username shown next to an alias; null when hidden or when there is no alias. */
  realName: (account: Named) => string | null;
  /** The name of the account with this ID, for rows that only carry the ID and username. */
  nameOf: (id: number, fallback: string) => string;
  /** `text` with every account username replaced, for logs and messages. */
  scrub: (text: string) => string;
}

const open: Privacy = {
  hidden: false,
  name: (account) => account.alias || account.username,
  username: (account) => account.username,
  realName: (account) => (account.alias ? account.username : null),
  nameOf: (_id, fallback) => fallback,
  scrub: (text) => text,
};

const PrivacyContext = createContext<Privacy>(open);

export function PrivacyProvider({
  hidden,
  accounts,
  children,
}: {
  hidden: boolean;
  accounts: Account[];
  children: ReactNode;
}) {
  const t = useT();
  const value = useMemo<Privacy>(() => {
    if (!hidden) return open;
    const hiddenName = (account: Pick<Account, "id" | "alias">) =>
      account.alias || t("privacy.account", { id: account.id });
    const byId = new Map(accounts.map((account) => [account.id, account]));
    const byName = new Map(
      accounts.map((account) => [
        account.username.toLowerCase(),
        hiddenName(account),
      ]),
    );
    return {
      hidden: true,
      name: hiddenName,
      username: hiddenName,
      realName: () => null,
      nameOf: (id) => {
        const account = byId.get(id);
        return account ? hiddenName(account) : t("privacy.account", { id });
      },
      // Usernames are letters, digits and underscores, so whole words are looked up.
      scrub: (text) =>
        text.replace(
          /[A-Za-z0-9_]{3,}/g,
          (word) => byName.get(word.toLowerCase()) ?? word,
        ),
    };
  }, [hidden, accounts, t]);
  return (
    <PrivacyContext.Provider value={value}>{children}</PrivacyContext.Provider>
  );
}

export const usePrivacy = () => useContext(PrivacyContext);
