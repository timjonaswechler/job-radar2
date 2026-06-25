import { Postings } from "@/features/postings/components/postings";
import { PostingsLayout } from "@/features/postings/components/postings-layout";
import { useJobPostings } from "@/features/postings/use-job-postings";

export function PostingsFeature() {
  const { postings, loading, error, refresh } = useJobPostings();

  return (
    <PostingsLayout>
      <Postings
        error={error}
        loading={loading}
        postings={postings}
        onRefresh={refresh}
      />
    </PostingsLayout>
  );
}
