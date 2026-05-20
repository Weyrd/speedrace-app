import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import frCommon from "../../public/locales/fr/common.json";
import frApp from "../../public/locales/fr/app.json";
import enCommon from "../../public/locales/en/common.json";
import enApp from "../../public/locales/en/app.json";

i18n.use(initReactI18next).init({
  resources: {
    fr: { common: frCommon, app: frApp },
    en: { common: enCommon, app: enApp },
  },
  lng: "fr",
  fallbackLng: "fr",
  defaultNS: "common",
  ns: ["common", "app"],
  interpolation: { escapeValue: false },
});

export default i18n;
