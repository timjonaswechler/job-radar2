import * as React from "react"
import { useTranslation } from "react-i18next"

import {
  LocaleProviderContext,
  type DateInput,
} from "@/context/locale-provider-context"
import { dateSelectorI18nByLanguage } from "@/lib/i18n/date-selector"
import { getSupportedLanguage, languageMetadata } from "@/lib/i18n/language"
import type { SupportedLanguage } from "@/lib/i18n/resources"

type LocaleProviderProps = {
  children: React.ReactNode
}

export function LocaleProvider({ children }: LocaleProviderProps) {
  const { i18n } = useTranslation()
  const locale = getSupportedLanguage(i18n.resolvedLanguage ?? i18n.language)
  const metadata = languageMetadata[locale]

  React.useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])

  const setLocale = React.useCallback(
    async (nextLocale: SupportedLanguage) => {
      await i18n.changeLanguage(nextLocale)
    },
    [i18n]
  )

  const formatDate = React.useCallback(
    (value: DateInput, options: Intl.DateTimeFormatOptions = {}) => {
      return new Intl.DateTimeFormat(metadata.intlLocale, options).format(
        new Date(value)
      )
    },
    [metadata.intlLocale]
  )

  const formatDateTime = React.useCallback(
    (
      value: DateInput,
      options: Intl.DateTimeFormatOptions = {
        dateStyle: "medium",
        timeStyle: "short",
      }
    ) => {
      return formatDate(value, options)
    },
    [formatDate]
  )

  const value = React.useMemo(
    () => ({
      locale,
      metadata,
      intlLocale: metadata.intlLocale,
      dateFormat: metadata.dateFormat,
      weekStartsOn: metadata.weekStartsOn,
      dateSelectorI18n: dateSelectorI18nByLanguage[locale],
      setLocale,
      formatDate,
      formatDateTime,
    }),
    [formatDate, formatDateTime, locale, metadata, setLocale]
  )

  return (
    <LocaleProviderContext.Provider value={value}>
      {children}
    </LocaleProviderContext.Provider>
  )
}
