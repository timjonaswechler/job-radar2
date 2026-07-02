pub type EmbeddedRegistryDocument<'a> = (&'a str, &'a str);

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
