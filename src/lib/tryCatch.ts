type Success<T> = {
  data: T;
  error: null;
};

type Failure<TError> = {
  data: null;
  error: TError;
};

export type Result<T, TError = Error> = Success<T> | Failure<TError>;

export async function tryCatch<T, TError = Error>(
  promise: Promise<T>,
): Promise<Result<T, TError>>;

export function tryCatch<T, TError = Error>(fn: () => T): Result<T, TError>;

export function tryCatch<T, TError = Error>(
  promiseOrFn: Promise<T> | (() => T),
): Promise<Result<T, TError>> | Result<T, TError> {
  if (typeof promiseOrFn === "object" && "then" in promiseOrFn) {
    return handlePromise(promiseOrFn);
  }
  return handleSync(promiseOrFn);
}

async function handlePromise<T, TError = Error>(
  promise: Promise<T>,
): Promise<Result<T, TError>> {
  try {
    const data = await promise;
    return { data, error: null };
  } catch (error) {
    return { data: null, error: error as TError };
  }
}

function handleSync<T, TError = Error>(fn: () => T): Result<T, TError> {
  try {
    const data = fn();
    return { data, error: null };
  } catch (error) {
    return { data: null, error: error as TError };
  }
}
