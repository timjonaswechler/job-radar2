pub type EmbeddedSourceRegistryDocument<'a> = (&'a str, &'a str);

pub const BUILTIN_SOURCE_PROFILE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[
    (
        "source-profiles/builtin/ashby.json",
        include_str!("../../../resources/profiles/ashby.json"),
    ),
    (
        "source-profiles/builtin/greenhouse.json",
        include_str!("../../../resources/profiles/greenhouse.json"),
    ),
    (
        "source-profiles/builtin/lever.json",
        include_str!("../../../resources/profiles/lever.json"),
    ),
    (
        "source-profiles/builtin/magnolia_esmp_job_search.json",
        include_str!("../../../resources/profiles/magnolia_esmp_job_search.json"),
    ),
    (
        "source-profiles/builtin/muz_global_jobboard.json",
        include_str!("../../../resources/profiles/muz_global_jobboard.json"),
    ),
    (
        "source-profiles/builtin/personio.json",
        include_str!("../../../resources/profiles/personio.json"),
    ),
    (
        "source-profiles/builtin/phenom.json",
        include_str!("../../../resources/profiles/phenom.json"),
    ),
    (
        "source-profiles/builtin/stepstone_de.json",
        include_str!("../../../resources/profiles/stepstone_de.json"),
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

pub const BUILTIN_SOURCE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[(
    "sources/builtin/stepstone_de.json",
    include_str!("../../../resources/sources/stepstone_de.json"),
)];
