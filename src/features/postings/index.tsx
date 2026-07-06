import { PostingsPageFrame } from "@/features/postings/workspace/postings-page-frame";
import { PostingsWorkspaceView } from "@/features/postings/workspace/postings-workspace-view";

export function PostingsFeature() {
  return (
    <PostingsPageFrame>
      <PostingsWorkspaceView />
    </PostingsPageFrame>
  );
}
