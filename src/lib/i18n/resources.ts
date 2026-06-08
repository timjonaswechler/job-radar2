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

type PreviousDepth = [never, 0, 1, 2, 3, 4, 5]

type DotNestedKeys<T, Depth extends number = 5> = [Depth] extends [never]
  ? never
  : {
      [Key in Extract<keyof T, string>]: T[Key] extends Record<string, unknown>
        ? Key | `${Key}.${DotNestedKeys<T[Key], PreviousDepth[Depth]>}`
        : Key
    }[Extract<keyof T, string>]

export type TranslationKey = DotNestedKeys<(typeof resources)["de"]["translation"]>
