"use client";
import { Badge } from "@/components/reui/badge"
import { Search } from "lucide-react";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"

import type { Mail } from "./data";
import { MailList } from "./mail-list";

interface MailInboxProps {
  mails: Mail[];
  onSelectMail?: (mail: Mail) => void;
}

export function MailInbox({ mails, onSelectMail }: MailInboxProps) {
  const pinnedMails = mails.filter((mail) => mail.isPinned);
  const unpinnedMails = mails.filter((mail) => !mail.isPinned);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 py-3 ">
      <Tabs defaultValue="inbox">
        <TabsList variant="line" className="mb-3.5 w-full pr-4">
          <TabsTrigger value="inbox" className="gap-2">
            All
            <Badge size="sm">
              12
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="drafts" className="gap-2">
            Drafts
            <Badge size="sm">
              3
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="sent" className="gap-2">
            Sent
          </TabsTrigger>
          <TabsTrigger value="spam" className="gap-2">
            Spam
            <Badge size="sm">
              24
            </Badge>
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div className="px-2 pr-4">
        <InputGroup className="h-7 w-full rounded-md">
          <InputGroupInput className="h-7" placeholder="Search..." />
          <InputGroupAddon>
            <Search />
          </InputGroupAddon>
        </InputGroup>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-1.5">
        <MailList
          groups={[
            {
              id: "pinned",
              title: "Pinned",
              items: pinnedMails,
            },
            {
              id: "inbox",
              title: "Inbox",
              items: unpinnedMails,
            },
          ]}
          onSelectMail={onSelectMail}
        />
      </div>
    </div>
  );
}
