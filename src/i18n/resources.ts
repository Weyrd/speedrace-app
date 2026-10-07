import common from "../locales/fr/common.json";
import app from "../locales/fr/app.json";
import settings from "../locales/fr/settings.json";
import streamErrors from "../locales/fr/streamErrors.json";

const resources = {
  common,
  app,
  settings,
  streamErrors,
} as const;

export default resources;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common";
    resources: typeof resources;
  }
}
