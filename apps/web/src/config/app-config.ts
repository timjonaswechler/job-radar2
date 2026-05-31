import packageJson from "../../package.json"

const currentYear = new Date().getFullYear()

export const APP_CONFIG = {
  name: "Job Radar",
  version: packageJson.version,
  copyright: `© ${currentYear}, Job Radar.`,
  meta: {
    title: "Job Radar",
    description:
      "Lokale Desktop-App zum Verwalten von Stellenanzeigen, Bewerbungen, Suchläufen und Erinnerungen.",
  },
}
