export type SupportedLocale =
  "en" | "de" | "fr" | "es" | "uk" | "ja" | "zh" | "hi" | "pl" | "no" | "fi" | "sv";

export interface LocaleOption {
  id: SupportedLocale;
  label: string;
}

/** Native-language display names, shown in the language picker. */
export const SUPPORTED_LOCALES: LocaleOption[] = [
  { id: "en", label: "English" },
  { id: "de", label: "Deutsch" },
  { id: "fr", label: "Français" },
  { id: "es", label: "Español" },
  { id: "uk", label: "Українська" },
  { id: "ja", label: "日本語" },
  { id: "zh", label: "中文" },
  { id: "hi", label: "हिन्दी" },
  { id: "pl", label: "Polski" },
  { id: "no", label: "Norsk" },
  { id: "fi", label: "Suomi" },
  { id: "sv", label: "Svenska" },
];
