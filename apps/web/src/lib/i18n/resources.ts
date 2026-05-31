import { de } from "./de"
import { en } from "./en"

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

type DotNestedKeys<T> = {
  [Key in Extract<keyof T, string>]: T[Key] extends Record<string, unknown>
    ? `${Key}.${DotNestedKeys<T[Key]>}`
    : Key
}[Extract<keyof T, string>]

export type TranslationKey = DotNestedKeys<(typeof resources)["de"]["translation"]>
