import { useTranslation } from "react-i18next";


export default function Footer() {
  const { t } = useTranslation("common");
  return (
    <div className="w-full flex justify-center border-t border-border">
      <p className="text-2xs text-dim tracking-wide font-mono py-2">
        {t("version")}
      </p>
    </div>
  );
}