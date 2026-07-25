export type SupportedLocale = "en" | "de" | "fr" | "es" | "uk";

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
];
