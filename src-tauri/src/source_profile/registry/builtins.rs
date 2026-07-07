pub type EmbeddedRegistryDocument<'a> = (&'a str, &'a str);

pub(crate) const BUILT_IN_ORIGIN: &str = "built_in";
pub(crate) const CUSTOM_ORIGIN: &str = "custom";

pub const BUILTIN_SOURCE_PROFILE_JSON_FILES: &[EmbeddedRegistryDocument<'static>] = &[
    (
        "source-profiles/builtin/greenhouse.json",
        include_str!("../../../resources/profiles/greenhouse.json"),
    ),
    (
        "source-profiles/builtin/successfactors.json",
        include_str!("../../../resources/profiles/successfactors.json"),
    ),
    (
        "source-profiles/builtin/workday.json",
        include_str!("../../../resources/profiles/workday.json"),
    ),
];

pub const BUILTIN_SOURCE_JSON_FILES: &[EmbeddedRegistryDocument<'static>] = &[];

pub(crate) const BUILTIN_SOURCE_PROFILE_FIXTURE_FILES: &[EmbeddedRegistryDocument<'static>] = &[
    (
        "source-profile-fixtures/builtin/greenhouse/fixture.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/greenhouse/fixture.json"),
    ),
    (
        "source-profile-fixtures/builtin/greenhouse/responses/posting-discovery-response.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/greenhouse/responses/posting-discovery-response.json"),
    ),
    (
        "source-profile-fixtures/builtin/greenhouse/responses/posting-detail-9001-response.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/greenhouse/responses/posting-detail-9001-response.json"),
    ),
    (
        "source-profile-fixtures/builtin/successfactors/fixture.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/successfactors/fixture.json"),
    ),
    (
        "source-profile-fixtures/builtin/successfactors/responses/posting-discovery-sitemap.xml",
        include_str!("../../../resources/source-profile-fixtures/builtin/successfactors/responses/posting-discovery-sitemap.xml"),
    ),
    (
        "source-profile-fixtures/builtin/successfactors/responses/posting-detail-1001-primary.html",
        include_str!("../../../resources/source-profile-fixtures/builtin/successfactors/responses/posting-detail-1001-primary.html"),
    ),
    (
        "source-profile-fixtures/builtin/workday/fixture.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/workday/fixture.json"),
    ),
    (
        "source-profile-fixtures/builtin/workday/responses/posting-discovery-page-0-response.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/workday/responses/posting-discovery-page-0-response.json"),
    ),
    (
        "source-profile-fixtures/builtin/workday/responses/posting-detail-jr-1001-response.json",
        include_str!("../../../resources/source-profile-fixtures/builtin/workday/responses/posting-detail-jr-1001-response.json"),
    ),
];
