import { type ReactNode } from "react";

import {
  ArchiveIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  EllipsisVerticalIcon,
  ForwardIcon,
  MailOpenIcon,
  PinIcon,
  ReplyAllIcon,
  ReplyIcon,
  TagIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

export function PreviewToolbar() {
  return (
    <div className="flex items-center gap-3">
      <div className="flex items-center gap-3">
        <DisabledToolbarButton label="Detail schließen folgt">
          <XIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <Separator
          className="h-4 data-vertical:self-center"
          orientation="vertical"
        />
        <div className="flex items-center gap-0">
          <DisabledToolbarButton label="Vorherige Anzeige folgt">
            <ChevronLeftIcon aria-hidden="true" />
          </DisabledToolbarButton>
          <DisabledToolbarButton label="Nächste Anzeige folgt">
            <ChevronRightIcon aria-hidden="true" />
          </DisabledToolbarButton>
        </div>
      </div>

      <div className="ml-auto flex items-center gap-2">
        <DisabledToolbarButton label="Anzeige anpinnen folgt">
          <PinIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <DisabledToolbarButton label="Archivieren folgt">
          <ArchiveIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <DisabledToolbarButton label="Notiz oder Antwort folgt">
          <ReplyIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <MoreActionsMenu />
        <Separator
          className="h-4 data-vertical:self-center"
          orientation="vertical"
        />
        <DisabledToolbarButton label="Entfernen folgt">
          <Trash2Icon aria-hidden="true" className="text-destructive" />
        </DisabledToolbarButton>
      </div>
    </div>
  );
}

function DisabledToolbarButton({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <span className="inline-flex size-7">
            <Button type="button" variant="ghost" size="icon" disabled>
              {children}
              <span className="sr-only">{label}</span>
            </Button>
          </span>
        }
      />
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function MoreActionsMenu() {
  return (
    <Tooltip>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button type="button" variant="ghost" size="icon-sm">
              <EllipsisVerticalIcon aria-hidden="true" />
              <span className="sr-only">Weitere Aktionen</span>
            </Button>
          }
        />
        <DropdownMenuContent align="end" className="w-56">
          <DropdownMenuGroup>
            <DropdownMenuItem disabled>
              <ReplyAllIcon aria-hidden="true" />
              Notiz hinzufügen folgt
            </DropdownMenuItem>
            <DropdownMenuItem disabled>
              <ForwardIcon aria-hidden="true" />
              Teilen folgt
            </DropdownMenuItem>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem disabled>
              <MailOpenIcon aria-hidden="true" />
              Als ungelesen markieren folgt
            </DropdownMenuItem>
            <DropdownMenuItem disabled>
              <TagIcon aria-hidden="true" />
              Label hinzufügen folgt
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <TooltipContent>Weitere Aktionen</TooltipContent>
    </Tooltip>
  );
}
