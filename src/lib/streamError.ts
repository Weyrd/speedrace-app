import type { TFunction } from "i18next";
import type { StreamError } from "../types";

export type PublishError = StreamError | Error | string;

function isStreamError(error: PublishError): error is StreamError {
  return typeof error === "object" && "code" in error;
}

export function streamErrorMessage(
  error: PublishError,
  t: TFunction<"app">,
  tError: TFunction<"streamErrors">,
): string {
  if (!isStreamError(error)) return t("stream.error_start");
  return tError(error.code, {
    detail: error.detail,
    defaultValue: t("stream.error_start"),
  });
}
