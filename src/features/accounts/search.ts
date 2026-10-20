import type { Account } from "@/generated/contracts";

export function searchTerms(search: string, separators: string) {
  const split = new Set(separators);
  const terms: string[] = [];
  let term = "";
  for (const char of search.toLowerCase()) {
    if (split.has(char)) {
      if (term.trim()) terms.push(term.trim());
      term = "";
    } else term += char;
  }
  if (term.trim()) terms.push(term.trim());
  return terms;
}

/** The first search term this account matches (by username, alias or own field), or -1. */
export function matchRank(
  account: Pick<Account, "username" | "alias" | "fields">,
  terms: string[],
) {
  const alias = account.alias.toLowerCase();
  // The user's own fields are found by value ("EU") or as "name: value" ("server: eu").
  const fields = account.fields.flatMap((field) => [
    field.value.toLowerCase(),
    `${field.name}: ${field.value}`.toLowerCase(),
  ]);
  return terms.findIndex(
    (term) =>
      account.username.toLowerCase().includes(term) ||
      (alias !== "" && alias.includes(term)) ||
      fields.some((field) => field.includes(term)),
  );
}

export function searchAccounts(
  accounts: Account[],
  search: string,
  separators: string,
) {
  const terms = searchTerms(search, separators);
  if (!terms.length) return accounts;
  const groups = terms.map(() => [] as Account[]);
  for (const account of accounts) {
    const index = matchRank(account, terms);
    if (index !== -1) groups[index].push(account);
  }
  return groups.flat();
}
