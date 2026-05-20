import common from "../../public/locales/fr/common.json";
import app from "../../public/locales/fr/app.json";

const resources = {
  common,
  app,
} as const;

export default resources;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common";
    resources: typeof resources;
  }
}
