# Load browser profiles from declarative profile definitions

Job Radar can register browser profiles from declarative profile definitions. Built-in definitions live with the app, while user- or agent-created definitions live in the app data directory; definitions may describe profile metadata, URL patterns, parameter schemas, and extraction rules, but they do not contain arbitrary executable user code. This keeps browser-based sources extensible without turning local profile files into an unsandboxed plugin system.
