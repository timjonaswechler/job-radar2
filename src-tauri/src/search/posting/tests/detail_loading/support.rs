pub(super) use super::super::*;

pub(super) fn direct_json_detail_step(fetch_url: &str, accept_when: Option<Value>) -> Value {
    let mut strategy = json!({
        "key": "json_detail",
        "fetch": {
            "mode": "http",
            "method": "GET",
            "url": fetch_url,
            "timeoutMs": 1000
        },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
            "fields": {
                "descriptionText": {
                    "type": "json_path",
                    "jsonPath": "$.description",
                    "cardinality": "one"
                }
            }
        }
    });
    if let Some(accept_when) = accept_when {
        strategy["acceptWhen"] = accept_when;
    }
    json!({ "strategies": [strategy] })
}

pub(super) fn collection_json_detail_step(fetch_url: &str) -> Value {
    json!({
        "strategies": [{
            "key": "collection_detail",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": fetch_url,
                "timeoutMs": 1000
            },
            "parse": { "type": "json" },
            "select": { "type": "json_path", "jsonPath": "$.jobs" },
            "match": {
                "type": "equal",
                "left": {
                    "type": "json_path",
                    "jsonPath": "$.id",
                    "cardinality": "one"
                },
                "right": {
                    "type": "posting_meta",
                    "key": "jobId",
                    "cardinality": "one"
                }
            },
            "extract": {
                "fields": {
                    "descriptionText": {
                        "type": "json_path",
                        "jsonPath": "$.description",
                        "cardinality": "one"
                    }
                }
            }
        }]
    })
}

pub(super) fn browser_html_detail_step(fetch_url: &str) -> Value {
    json!({
        "strategies": [{
            "key": "browser_detail",
            "fetch": {
                "mode": "browser",
                "url": fetch_url,
                "timeoutMs": 1000
            },
            "parse": { "type": "html" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "descriptionText": {
                        "type": "css_text",
                        "selector": ".description",
                        "cardinality": "first"
                    }
                }
            }
        }]
    })
}
