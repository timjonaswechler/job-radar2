use job_radar_lib::{
    compile_parse, compile_select, select_descriptors, validate_select_registration_keys,
    BrowserRenderedText, CompiledSelect, ParseInput, ParseInputKind, ParseType, ParsedDocument,
    Select, SelectCompileContext, SelectKind, SelectPhase, SelectPlacement, SelectRegistryError,
    SelectTypeFragment, SelectedItem,
};

fn context(document_type: ParseType) -> SelectCompileContext {
    SelectCompileContext {
        document_type,
        phase: SelectPhase::Discovery,
        placement: SelectPlacement::Strategy,
    }
}

fn authored(value: serde_json::Value) -> Select {
    serde_json::from_value(value).unwrap()
}

#[test]
fn select_family_has_exact_cross_layer_registration_parity() {
    let expected = vec![
        "document",
        "json_path",
        "xml_element",
        "xml_text",
        "css",
        "sitemap_urls",
    ];
    let schema = select_schema_keys();
    let serde = SelectKind::ALL
        .iter()
        .map(|kind| kind.key().to_string())
        .collect::<Vec<_>>();
    let fragments = SelectTypeFragment::ALL
        .iter()
        .map(|kind| kind.key().to_string())
        .collect::<Vec<_>>();
    let registrations = select_descriptors()
        .iter()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(schema, expected);
    assert_eq!(serde, expected);
    assert_eq!(fragments, expected);
    assert_eq!(registrations, expected);
    assert_eq!(
        validate_select_registration_keys(&schema, &serde, &fragments, &registrations),
        Ok(())
    );
    assert_eq!(
        validate_select_registration_keys(&schema, &serde[..5], &fragments, &registrations),
        Err(SelectRegistryError::Missing {
            layer: "serde",
            keys: vec!["sitemap_urls".to_string()]
        })
    );
    let mut duplicate = registrations.clone();
    duplicate.push("css".to_string());
    assert_eq!(
        validate_select_registration_keys(&schema, &serde, &fragments, &duplicate),
        Err(SelectRegistryError::Duplicate {
            layer: "registration",
            keys: vec!["css".to_string()]
        })
    );
    assert_eq!(
        validate_select_registration_keys(&schema, &serde, &fragments[..5], &registrations),
        Err(SelectRegistryError::Missing {
            layer: "fragment",
            keys: vec!["sitemap_urls".to_string()]
        })
    );
    let mut duplicate_fragments = fragments.clone();
    duplicate_fragments.push("css".to_string());
    assert_eq!(
        validate_select_registration_keys(&schema, &serde, &duplicate_fragments, &registrations),
        Err(SelectRegistryError::Duplicate {
            layer: "fragment",
            keys: vec!["css".to_string()]
        })
    );
}

#[test]
fn six_compiled_variants_return_one_ordered_selected_sequence() {
    let json_document = ParsedDocument::Json(serde_json::json!({"jobs":[{"id":1},{"id":2}]}));
    let json = compile_select(
        &authored(serde_json::json!({"type":"json_path","jsonPath":"$.jobs"})),
        context(ParseType::Json),
    )
    .unwrap();
    let selected = json.select(&json_document).unwrap();
    assert!(
        matches!(selected.as_slice(), [SelectedItem::Json(value)] if value.as_array().unwrap().len() == 2)
    );

    let document = compile_select(
        &authored(serde_json::json!({"type":"document"})),
        context(ParseType::Json),
    )
    .unwrap();
    assert_eq!(document.select(&json_document).unwrap().len(), 1);

    let xml_document = ParsedDocument::Xml(roxmltree::Document::parse(
        "<root><job><title>First</title></job><job><title>Second <b>Role</b></title></job></root>"
    ).unwrap());
    let elements = compile_select(
        &authored(serde_json::json!({"type":"xml_element","element":"job"})),
        context(ParseType::Xml),
    )
    .unwrap();
    let selected = elements.select(&xml_document).unwrap();
    assert_eq!(
        selected
            .as_slice()
            .iter()
            .map(|item| match item {
                SelectedItem::Xml(node) => node.attribute("id").unwrap_or(node.tag_name().name()),
                _ => "",
            })
            .collect::<Vec<_>>(),
        vec!["job", "job"]
    );

    let texts = compile_select(
        &authored(serde_json::json!({"type":"xml_text","textPath":"/root//job/title/"})),
        context(ParseType::Xml),
    )
    .unwrap();
    let values = texts
        .select(&xml_document)
        .unwrap()
        .into_vec()
        .into_iter()
        .map(|item| match item {
            SelectedItem::Text(value) => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec!["First", "Second Role"]);

    let html_parse =
        compile_parse(&authored_parse("html"), ParseInputKind::BrowserRendered).unwrap();
    let html_document = html_parse
        .parse(ParseInput::BrowserRendered(BrowserRenderedText::new(
            "<main><article>A</article><article>B</article></main>",
        )))
        .unwrap();
    let css = compile_select(
        &authored(serde_json::json!({"type":"css","selector":"article"})),
        context(ParseType::Html),
    )
    .unwrap();
    assert_eq!(css.select(&html_document).unwrap().len(), 2);

    let sitemap = compile_select(
        &authored(serde_json::json!({"type":"sitemap_urls","urlPattern":"/jobs/"})),
        SelectCompileContext {
            document_type: ParseType::Xml,
            phase: SelectPhase::Discovery,
            placement: SelectPlacement::SitemapPosting,
        },
    )
    .unwrap();
    let sitemap_document = ParsedDocument::Xml(roxmltree::Document::parse("<urlset><url><loc> https://example.test/jobs/2 </loc></url><url><loc>https://example.test/other</loc></url><url><loc>https://example.test/jobs/1</loc></url></urlset>").unwrap());
    let values = sitemap
        .select(&sitemap_document)
        .unwrap()
        .into_vec()
        .into_iter()
        .map(|item| match item {
            SelectedItem::Text(value) => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec!["https://example.test/jobs/2", "https://example.test/jobs/1"]
    );
}

#[test]
fn invalid_syntax_context_phase_and_placement_are_rejected_at_compile_time() {
    for (select, document_type) in [
        (
            serde_json::json!({"type":"json_path","jsonPath":"$.jobs[*]"}),
            ParseType::Json,
        ),
        (
            serde_json::json!({"type":"css","selector":"article["}),
            ParseType::Html,
        ),
    ] {
        assert!(compile_select(&authored(select), context(document_type)).is_err());
    }
    let bad_regex = authored(serde_json::json!({"type":"sitemap_urls","urlPattern":"["}));
    assert!(compile_select(
        &bad_regex,
        SelectCompileContext {
            document_type: ParseType::Xml,
            phase: SelectPhase::Discovery,
            placement: SelectPlacement::SitemapPosting
        }
    )
    .is_err());
    assert!(compile_select(
        &authored(serde_json::json!({"type":"css","selector":"article"})),
        context(ParseType::Json)
    )
    .is_err());
    let sitemap = authored(serde_json::json!({"type":"sitemap_urls"}));
    assert!(compile_select(
        &sitemap,
        SelectCompileContext {
            document_type: ParseType::Xml,
            phase: SelectPhase::Detail,
            placement: SelectPlacement::SitemapPosting
        }
    )
    .is_err());
    assert!(compile_select(&sitemap, context(ParseType::Xml)).is_err());
    assert!(compile_select(
        &authored(serde_json::json!({"type":"xml_element","element":"loc"})),
        SelectCompileContext {
            document_type: ParseType::Xml,
            phase: SelectPhase::Discovery,
            placement: SelectPlacement::SitemapChild
        }
    )
    .is_err());
}

#[test]
fn literal_xml_grammar_handles_current_slashes_direct_children_and_literal_names() {
    let document = ParsedDocument::Xml(roxmltree::Document::parse("<root><group><name>direct</name><nested><name>nested</name></nested></group><name>root</name></root>").unwrap());
    for path in [".", "", "///"] {
        let plan = compile_select(
            &authored(serde_json::json!({"type":"xml_text","textPath":path})),
            context(ParseType::Xml),
        )
        .unwrap();
        assert_eq!(plan.select(&document).unwrap().len(), 1);
    }
    let direct = compile_select(
        &authored(serde_json::json!({"type":"xml_text","textPath":"root/group/name"})),
        context(ParseType::Xml),
    )
    .unwrap();
    let values = direct
        .select(&document)
        .unwrap()
        .into_vec()
        .into_iter()
        .map(|item| match item {
            SelectedItem::Text(value) => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec!["direct"]);

    let first_step = compile_select(
        &authored(serde_json::json!({"type":"xml_text","textPath":"name"})),
        context(ParseType::Xml),
    )
    .unwrap();
    let values = first_step
        .select(&document)
        .unwrap()
        .into_vec()
        .into_iter()
        .map(|item| match item {
            SelectedItem::Text(value) => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec!["root"]);

    for literal in ["*", "@name", "name[1]", "../name"] {
        let plan = compile_select(
            &authored(serde_json::json!({"type":"xml_element","element":literal})),
            context(ParseType::Xml),
        )
        .unwrap();
        assert!(plan.select(&document).unwrap().is_empty());
    }
}

#[test]
fn compiled_serde_revalidates_css_and_regex_invariants() {
    serde_json::from_value::<CompiledSelect>(serde_json::json!({"type":"css","selector":"["}))
        .expect_err("invalid compiled CSS must be rejected");
    serde_json::from_value::<CompiledSelect>(
        serde_json::json!({"type":"sitemap_urls","urlPattern":"["}),
    )
    .expect_err("invalid compiled regex must be rejected");
}

fn authored_parse(parse_type: &str) -> job_radar_lib::Parse {
    serde_json::from_value(serde_json::json!({"type":parse_type})).unwrap()
}

fn select_schema_keys() -> Vec<String> {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../src/schema/profile-dsl/select.schema.json"
    ))
    .unwrap();
    schema["$defs"]["select"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let reference = entry["$ref"].as_str().unwrap();
            let definition = reference.rsplit('/').next().unwrap();
            schema["$defs"][definition]["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}
