import * as React from "react"

import type { DateSelectorI18nConfig } from "@/lib/i18n/date-selector"
import type { LanguageMetadata } from "@/lib/i18n/language"
import type { SupportedLanguage } from "@/lib/i18n/resources"

export type DateInput = Date | string | number

export type LocaleProviderState = {
  locale: SupportedLanguage
  metadata: LanguageMetadata
  intlLocale: string
  dateFormat: string
  weekStartsOn: LanguageMetadata["weekStartsOn"]
  dateSelectorI18n: DateSelectorI18nConfig
  setLocale: (locale: SupportedLanguage) => Promise<void>
  formatDate: (value: DateInput, options?: Intl.DateTimeFormatOptions) => string
  formatDateTime: (
    value: DateInput,
    options?: Intl.DateTimeFormatOptions
  ) => string
}

export const LocaleProviderContext = React.createContext<
  LocaleProviderState | undefined
>(undefined)

export function useLocale() {
  const context = React.useContext(LocaleProviderContext)

  if (context === undefined) {
    throw new Error("useLocale must be used within a LocaleProvider")
  }

  return context
}
