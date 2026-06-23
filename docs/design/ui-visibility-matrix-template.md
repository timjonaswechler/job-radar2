# UI Visibility Matrix Template

Use this before adding fields from backend/domain objects to the UI.

```txt
Screen / Component:
Primary user task:
Primary decision or next action:

Field | Visible? | Disclosure Level | Reason | Presentation
--- | --- | --- | --- | ---
name/title | yes | overview | identifies the item | primary text
status | yes | overview | shows whether action is needed | badge/chip
lastActivity | yes | overview | helps prioritize | relative time, absolute in tooltip
internalId | no | debug | implementation detail | debug/raw data only
rawJson | no | debug | maintenance only | collapsible raw view
```

Disclosure levels:

1. Overview
2. Row/Card
3. Detail Panel
4. Advanced
5. Debug/Raw Data
