pub type EmbeddedSourceRegistryDocument<'a> = (&'a str, &'a str);

pub const BUILTIN_SOURCE_PROFILE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[
    (
        "source-profiles/builtin/ashby.json",
        include_str!("../../../source-profiles/builtin/ashby.json"),
    ),
    (
        "source-profiles/builtin/greenhouse.json",
        include_str!("../../../source-profiles/builtin/greenhouse.json"),
    ),
    (
        "source-profiles/builtin/lever.json",
        include_str!("../../../source-profiles/builtin/lever.json"),
    ),
    (
        "source-profiles/builtin/magnolia_esmp_job_search.json",
        include_str!("../../../source-profiles/builtin/magnolia_esmp_job_search.json"),
    ),
    (
        "source-profiles/builtin/muz_global_jobboard.json",
        include_str!("../../../source-profiles/builtin/muz_global_jobboard.json"),
    ),
    (
        "source-profiles/builtin/personio.json",
        include_str!("../../../source-profiles/builtin/personio.json"),
    ),
    (
        "source-profiles/builtin/phenom.json",
        include_str!("../../../source-profiles/builtin/phenom.json"),
    ),
    (
        "source-profiles/builtin/stepstone_de.json",
        include_str!("../../../source-profiles/builtin/stepstone_de.json"),
    ),
    (
        "source-profiles/builtin/successfactors.json",
        include_str!("../../../source-profiles/builtin/successfactors.json"),
    ),
    (
        "source-profiles/builtin/workday.json",
        include_str!("../../../source-profiles/builtin/workday.json"),
    ),
];

pub const BUILTIN_SOURCE_JSON_FILES: &[EmbeddedSourceRegistryDocument<'static>] = &[(
    "sources/builtin/stepstone_de.json",
    include_str!("../../../sources/builtin/stepstone_de.json"),
)];
