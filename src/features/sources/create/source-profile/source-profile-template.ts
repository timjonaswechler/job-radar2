import type { JsonValue } from "@/lib/api/sources";

export const profileTemplateSnippet: JsonValue = {
  schemaVersion: 3,
  key: "example_profile",
  name: "Example Profile",
  kind: "generic",
  support: {
    level: "experimental",
    summary: "Startpunkt für ein neues deklaratives Source Profile.",
  },
  sourceConfigSchema: {
    type: "object",
    required: ["startUrl"],
    additionalProperties: false,
    properties: {
      startUrl: {
        type: "string",
        format: "uri",
        title: "Start URL",
      },
    },
  },
  accessPaths: [
    {
      key: "html_jobs",
      name: "HTML jobs page",
      discovery: {
        policy: { type: "first_accepted" },
        strategies: [
          {
            key: "jobs_html",
            fetch: {
              mode: "http",
              method: "GET",
              url: "{{sourceConfig:startUrl}}",
              timeoutMs: 10000,
            },
            parse: { type: "html" },
            select: { type: "css", selector: ".job" },
            extract: {
              fields: {
                title: {
                  type: "css_text",
                  selector: ".title",
                  cardinality: "one",
                },
                company: {
                  type: "template",
                  template: "{{source:name}}",
                  cardinality: "one",
                },
                url: {
                  type: "css_attribute",
                  selector: "a",
                  attribute: "href",
                  cardinality: "one",
                },
              },
            },
          },
        ],
      },
    },
  ],
};
