/**
 * Runs an operation and shows `success`, or the message built from its result, or the error.
 * Resolves once the operation has finished; it never rejects.
 */
export type RunAction = <T>(
  action: () => Promise<T>,
  success?: string | ((result: T) => string),
) => Promise<void>;
