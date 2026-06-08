import { supportedLanguages, type SupportedLanguage } from "./resources"

export type WeekStartsOn = 0 | 1

export type LanguageMetadata = {
  label: string
  flag: string
  intlLocale: string
  dateFormat: string
  weekStartsOn: WeekStartsOn
}

export const languageMetadata = {
  de: {
    label: "Deutsch",
    flag: "🇩🇪",
    intlLocale: "de-DE",
    dateFormat: "dd.MM.yyyy",
    weekStartsOn: 1,
  },
  en: {
    label: "English",
    flag: "🇺🇸",
    intlLocale: "en-US",
    dateFormat: "MM/dd/yyyy",
    weekStartsOn: 0,
  },
} satisfies Record<SupportedLanguage, LanguageMetadata>

export const languageOptions = supportedLanguages.map((value) => ({
  value,
  label: languageMetadata[value].label,
  flag: languageMetadata[value].flag,
}))

export function isSupportedLanguage(
  value: string | undefined
): value is SupportedLanguage {
  return supportedLanguages.includes(value as SupportedLanguage)
}

export function getSupportedLanguage(
  language: string | undefined
): SupportedLanguage {
  const languageOnly = language?.split("-")[0]
  return isSupportedLanguage(languageOnly) ? languageOnly : "de"
}
