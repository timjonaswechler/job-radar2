# Job Radar

Job Radar helps one person track job applications, reminders, and recurring job-search activity.

## Language

**Bewerbung**:
A tracked attempt to get a specific role at a specific company. A Bewerbung always belongs to exactly one Stellenanzeige, and a Stellenanzeige can have at most one Bewerbung. It begins when the user decides to actively pursue that Stellenanzeige, not merely when it looks interesting; the Bewerbung remains editable over time. Its phase-one statuses are: neu, unterlagen vorbereiten, beworben, rückmeldung, erstgespräch, technisches interview, angebot, abgelehnt, zurückgezogen, archiviert.
_Avoid_: Job, Anzeige, Vorgang

**Suchanfrage**:
A configured search the user wants Job Radar to run repeatedly. It contains one user-defined Suchbegriff plus optional criteria such as location, radius, selected Jobquellen, and search-specific exclusion terms; company-career Jobquellen may need more granular matching rules.
_Avoid_: Filter, Query, Quelle

**Suchbegriff**:
The user-defined text submitted to portal-like Jobquellen. A Suchbegriff is not a free boolean expression and is not the same as local Trefferregeln.
_Avoid_: Suchausdruck, Schlagwortliste, Filterregel

**Trefferregel**:
A local rule on a Suchanfrage used after findings are retrieved to decide whether a finding matches what the user is looking for. Phase-one Trefferregeln are title-focused, support contains and does-not-contain checks, and are combined conjunctively: all Trefferregeln on a Suchanfrage must match.
_Avoid_: Suchausdruck, Portal-Query

**Ausschlussbegriff**:
A simple word or phrase that makes a found Stellenanzeige irrelevant when it appears in the title, such as "Duales Studium". Ausschlussbegriffe can be global or specific to one Suchanfrage; matching findings are kept out of the Stellenanzeigen-Inbox. They are the simple user-facing form of exclusion Trefferregeln.
_Avoid_: Blacklist, Negativfilter

**Ausgeschlossener Treffer**:
A finding that matched an Ausschlussbegriff and is retained only temporarily for review or debugging. Ausgeschlossene Treffer do not belong to the Stellenanzeigen-Inbox.
_Avoid_: Stellenanzeige, Duplikat, Gelöschter Treffer

**Quellsystem**:
The kind of system behind a Jobquelle, such as SAP SuccessFactors, Greenhouse, Lever, StepStone, LinkedIn, a feed, or a custom career page pattern.
_Avoid_: Plattform, Quelle, Adapter

**Jobquelle**:
A configured place where Job Radar can search for Stellenanzeigen. A Jobquelle belongs to a Quellsystem and contains whatever configuration that Quellsystem needs, such as a feed URL or site URL.
_Avoid_: Plattform, Quelle, Suchanbieter

**Suchlauf**:
A bounded pass over one or more Suchanfragen to find potentially relevant job postings. A Suchlauf may take time deliberately to avoid aggressive scraping behavior; each Jobquelle may apply the Suchanfrage only as far as its Quellsystem allows.
_Avoid_: Scrape, Crawl, Sync

**Stellenanzeige**:
A deduplicated job posting found by a Suchlauf or entered manually. A Stellenanzeige is not a Bewerbung until the user decides to track an application for it. One Stellenanzeige can have many Fundstellen; it holds the main extracted plain-text description, not raw HTML. Its phase-one statuses are: neu, interessant, später ansehen, ausgeblendet, in bewerbung umgewandelt.
_Avoid_: Jobangebot, Bewerbung, Job

**Fundstelle**:
A concrete place where a Stellenanzeige was found, such as a Jobquelle result URL or company career-page URL. Multiple Fundstellen can point to the same Stellenanzeige and may contribute additional information; when no external ID exists, Job Radar treats company, exact normalized title, primary location or region, and work model as the practical identity of a Stellenanzeige. Title normalization only removes cosmetic differences such as gender suffixes, punctuation, whitespace, and casing; similar but meaningfully different titles are treated as different Stellenanzeigen.
_Avoid_: Quelle, Link, Duplikat

**Arbeitsmodell**:
The arrangement for where work happens for a Stellenanzeige, such as remote, hybrid, on-site, or unknown. Arbeitsmodell is distinct from location.
_Avoid_: Ort, Standort

**Stellenanzeigen-Inbox**:
The working list of Stellenanzeigen that still need a user decision. It can contain new findings from Suchläufe and manually added Stellenanzeigen; a user can mark a Stellenanzeige as interesting, keep it for later, hide it, or turn it into a Bewerbung.
_Avoid_: Suchlauf-Historie, Bewerbungsliste

**Erinnerung**:
A typed prompt for the user to do something manually, such as start the daily Suchlauf, follow up on a Bewerbung, attend an interview, or handle a custom task. Erinnerungen have a due time and can be marked done.
_Avoid_: Alarm, Cron Job, Notification

## Example dialogue

Dev: “Should the app automatically scrape all sources every few minutes?”
Domain expert: “No. One daily Suchlauf is enough, and Job Radar may remind me to start or review it. The Suchlauf works through configured Suchanfragen gradually.”
Dev: “Is a found job already a Bewerbung?”
Domain expert: “No. A Suchlauf finds Fundstellen. Job Radar deduplicates them into Stellenanzeigen. New or undecided Stellenanzeigen land in the Stellenanzeigen-Inbox. A Stellenanzeige only becomes a Bewerbung once I decide to track an application for that role.”
