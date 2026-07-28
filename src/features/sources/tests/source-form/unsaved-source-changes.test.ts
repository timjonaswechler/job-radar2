import assert from "node:assert/strict";

import { decideUnsavedSourceChangesClose } from "@/features/sources/source-form/use-unsaved-source-changes";
import { test } from "vitest";

test("unsaved source changes contract", async () => {
  assert.equal(
    decideUnsavedSourceChangesClose({ discardBlocked: true, isDirty: false }),
    "ignore",
  );
  assert.equal(
    decideUnsavedSourceChangesClose({ discardBlocked: true, isDirty: true }),
    "ignore",
  );
  assert.equal(
    decideUnsavedSourceChangesClose({ discardBlocked: false, isDirty: false }),
    "close",
  );
  assert.equal(
    decideUnsavedSourceChangesClose({ discardBlocked: false, isDirty: true }),
    "confirm",
  );
});
