export function withAbortSignal<T>(
  promise: PromiseLike<T> | T,
  signal: AbortSignal | null | undefined,
  createError: () => Error,
  disposeLateValue: ((value: T) => void | Promise<void>) | null = null,
): Promise<T> {
  const operation = Promise.resolve(promise);
  if (!signal) return operation;

  return new Promise<T>((resolve, reject) => {
    let settled = false;
    const onAbort = () => {
      if (settled) return;
      settled = true;
      signal.removeEventListener("abort", onAbort);
      if (disposeLateValue) {
        void operation.then((value) => disposeLateValue(value)).catch(() => {});
      }
      reject(createError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
    operation.then(
      (value) => {
        if (settled) return;
        settled = true;
        signal.removeEventListener("abort", onAbort);
        resolve(value);
      },
      (error) => {
        if (settled) return;
        settled = true;
        signal.removeEventListener("abort", onAbort);
        reject(error);
      },
    );
    if (signal.aborted) onAbort();
  });
}
