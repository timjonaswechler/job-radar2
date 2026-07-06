import { SourcesPageFrame } from "@/features/sources/workspace/sources-page-frame";
import { SourcesWorkspaceView } from "@/features/sources/workspace/sources-workspace-view";

export function SourcesFeature() {
  return (
    <SourcesPageFrame>
      <SourcesWorkspaceView />
    </SourcesPageFrame>
  );
}
