import { Postings } from "@/features/postings/components/postings";
import { PostingsLayout } from "@/features/postings/components/postings-layout";

export function PostingsFeature() {
  return (
    <PostingsLayout>
      <Postings />
    </PostingsLayout>
  );
}
