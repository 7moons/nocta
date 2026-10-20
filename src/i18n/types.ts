import type { en } from "./locales/en";

/** A message that depends on a `count` parameter, following `Intl.PluralRules` categories. */
export interface Plural {
  zero?: string;
  one?: string;
  two?: string;
  few?: string;
  many?: string;
  other: string;
}

// Every locale has the same keys as English. Values may differ in plural categories, since
// languages use different ones (English: one/other, Polish: one/few/many/other).
type Shape<T> = {
  [K in keyof T]: T[K] extends string
    ? string
    : T[K] extends { other: string }
      ? Plural
      : Shape<T[K]>;
};
export type Messages = Shape<typeof en>;

type Join<A extends string, B extends string> = `${A}.${B}`;
type Keys<T> = {
  [K in keyof T & string]: T[K] extends string
    ? K
    : T[K] extends { other: string }
      ? K
      : Join<K, Keys<T[K]>>;
}[keyof T & string];

/** Every translatable key, such as `launch.title`. */
export type MessageKey = Keys<typeof en>;
