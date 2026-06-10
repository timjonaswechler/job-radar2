import { de } from "./de"
import { en } from "./en"
import type { DotNestedLeafKeys } from "./types"

export const defaultNS = "translation" as const

export const resources = {
  de: {
    translation: de,
  },
  en: {
    translation: en,
  },
} as const

export const supportedLanguages = ["de", "en"] as const

export type SupportedLanguage = (typeof supportedLanguages)[number]

export type TranslationKey = DotNestedLeafKeys<
  (typeof resources)["de"]["translation"]
>
