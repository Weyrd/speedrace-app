import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import frCommon from "../locales/fr/common.json";
import frApp from "../locales/fr/app.json";
import frSettings from "../locales/fr/settings.json";
import enCommon from "../locales/en/common.json";
import enApp from "../locales/en/app.json";
import enSettings from "../locales/en/settings.json";

i18n.use(initReactI18next).init({
  resources: {
    fr: { common: frCommon, app: frApp, settings: frSettings },
    en: { common: enCommon, app: enApp, settings: enSettings },
  },
  lng: "en",
  fallbackLng: "en",
  defaultNS: "common",
  ns: ["common", "app", "settings"],
  interpolation: { escapeValue: false },
});

export default i18n;
