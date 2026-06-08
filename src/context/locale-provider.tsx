import * as React from "react"
import { useTranslation } from "react-i18next"

import {
  LocaleProviderContext,
  type DateInput,
} from "@/context/locale-provider-context"
import { getAppPreferences, setAppLanguage } from "@/lib/api/app-preferences"
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

  React.useEffect(() => {
    let cancelled = false

    void getAppPreferences()
      .then((preferences) => {
        if (cancelled) return
        if (preferences.language !== locale) {
          void i18n.changeLanguage(preferences.language)
        }
      })
      .catch((error) => {
        console.warn("Could not read language from SQLite", error)
      })

    return () => {
      cancelled = true
    }
    // Read persisted language once on provider mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i18n])

  const setLocale = React.useCallback(
    async (nextLocale: SupportedLanguage) => {
      await i18n.changeLanguage(nextLocale)
      await setAppLanguage(nextLocale)
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
