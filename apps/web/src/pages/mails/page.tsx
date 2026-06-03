import { mails } from "./_components/data";
import { MailComponent } from "./_components/mail";

export function MailsPage() {
  return (
    <div className="h-dvh min-h-0 overflow-hidden">
      <MailComponent mails={mails} />
    </div>
  );
}
