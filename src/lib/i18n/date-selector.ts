import type { SupportedLanguage } from "./resources"

export interface DateSelectorI18nConfig {
  selectDate: string
  apply: string
  cancel: string
  clear: string
  today: string
  filterTypes: {
    is: string
    before: string
    after: string
    between: string
  }
  periodTypes: {
    day: string
    month: string
    quarter: string
    halfYear: string
    year: string
  }
  months: string[]
  monthsShort: string[]
  quarters: string[]
  halfYears: string[]
  weekdays: string[]
  weekdaysShort: string[]
  placeholder: string
  rangePlaceholder: string
}

export const DEFAULT_DATE_SELECTOR_I18N: DateSelectorI18nConfig = {
  selectDate: "Select date",
  apply: "Apply",
  cancel: "Cancel",
  clear: "Clear",
  today: "Today",
  filterTypes: {
    is: "is",
    before: "before",
    after: "after",
    between: "between",
  },
  periodTypes: {
    day: "Day",
    month: "Month",
    quarter: "Quarter",
    halfYear: "Half-year",
    year: "Year",
  },
  months: [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ],
  monthsShort: [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ],
  quarters: ["Q1", "Q2", "Q3", "Q4"],
  halfYears: ["H1", "H2"],
  weekdays: [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
  ],
  weekdaysShort: ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
  placeholder: "Select date...",
  rangePlaceholder: "Select date range...",
}

function createDateSelectorI18nConfig(
  translations: Partial<DateSelectorI18nConfig>
): DateSelectorI18nConfig {
  return { ...DEFAULT_DATE_SELECTOR_I18N, ...translations }
}

const deDateSelectorI18n = createDateSelectorI18nConfig({
  selectDate: "Datum auswählen",
  apply: "Anwenden",
  cancel: "Abbrechen",
  clear: "Löschen",
  today: "Heute",
  filterTypes: {
    is: "ist",
    before: "vor",
    after: "nach",
    between: "zwischen",
  },
  periodTypes: {
    day: "Tag",
    month: "Monat",
    quarter: "Quartal",
    halfYear: "Halbjahr",
    year: "Jahr",
  },
  months: [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
  ],
  monthsShort: [
    "Jan",
    "Feb",
    "Mär",
    "Apr",
    "Mai",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Okt",
    "Nov",
    "Dez",
  ],
  quarters: ["Q1", "Q2", "Q3", "Q4"],
  halfYears: ["H1", "H2"],
  weekdays: [
    "Sonntag",
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
  ],
  weekdaysShort: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
  placeholder: "Datum auswählen...",
  rangePlaceholder: "Datumsbereich auswählen...",
})

export const dateSelectorI18nByLanguage = {
  de: deDateSelectorI18n,
  en: DEFAULT_DATE_SELECTOR_I18N,
} satisfies Record<SupportedLanguage, DateSelectorI18nConfig>
