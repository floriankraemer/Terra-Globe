import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.js";
import de from "./locales/de.js";
import fr from "./locales/fr.js";
import es from "./locales/es.js";
import uk from "./locales/uk.js";
import type { SupportedLocale } from "./types.js";

// `en` is the source of truth for translation keys - typing every other
// locale against it means a missing/misspelled key fails the build instead
// of silently falling back to English at runtime.
type Translation = typeof en;
const resources: Record<SupportedLocale, { translation: Translation }> = {
  en: { translation: en },
  de: { translation: de },
  fr: { translation: fr },
  es: { translation: es },
  uk: { translation: uk },
};

void i18next.use(initReactI18next).init({
  resources,
  lng: "en",
  fallbackLng: "en",
  interpolation: {
    // React already escapes interpolated values when rendering JSX.
    escapeValue: false,
  },
});

export default i18next;
