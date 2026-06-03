"use client";

import * as React from "react";

import { Drawer, DrawerContent, DrawerDescription, DrawerTitle } from "@/components/ui/drawer";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { useSidebar } from "@/components/ui/sidebar";


import type { Mail } from "./data";
import { MailInbox } from "./mail-inbox";
import {
  MAIL_DETAIL_PANEL_ID,
  MAIL_LIST_PANEL_ID,
} from "./mail-layout-config";
import { MailView } from "./mail-view";
import { useMail } from "./use-mail";

interface MailProps {
  mails: Mail[];
}

export function MailComponent({ mails }: MailProps) {
  const { isMobile } = useSidebar();

  return isMobile ? (
    <MailMobileLayout mails={mails} />
  ) : (
    <MailDesktopLayout mails={mails} />
  );
}

function MailMobileLayout({ mails }: Pick<MailProps, "mails">) {
  const [mail] = useMail();
  const [isMailOpen, setIsMailOpen] = React.useState(false);
  const selectedMail = mails.find((item) => item.id === mail.selected) || null;

  return (
    <>
      <MailInbox mails={mails} onSelectMail={() => setIsMailOpen(true)} />

      <Drawer open={isMailOpen} onOpenChange={setIsMailOpen}>
        <DrawerContent>
          <DrawerTitle className="sr-only">Mail message</DrawerTitle>
          <DrawerDescription className="sr-only">Read the selected email message</DrawerDescription>
          <MailView mail={selectedMail}/>
        </DrawerContent>
      </Drawer>
    </>
  );
}

function MailDesktopLayout({ mails}: MailProps) {
  const [mail] = useMail();

  return (
    <ResizablePanelGroup
      orientation="horizontal"
      className="h-full"
    >
      <ResizablePanel id={MAIL_LIST_PANEL_ID}  minSize="30%" defaultSize="30%" className="min-h-0 ">
        <MailInbox mails={mails} />
      </ResizablePanel>
      <ResizableHandle withHandle />
      <ResizablePanel id={MAIL_DETAIL_PANEL_ID}  minSize="30%" defaultSize="70%" className="min-h-0">
        <MailView mail={mails.find((item) => item.id === mail.selected) || null} />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
