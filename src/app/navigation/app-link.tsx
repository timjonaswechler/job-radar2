import type { ComponentProps, MouseEvent } from "react";

import { navigateTo } from "@/app/navigation/path";

type AppLinkProps = Omit<ComponentProps<"a">, "href"> & {
  href: string;
};

export function AppLink({ href, onClick, ...props }: AppLinkProps) {
  const handleClick = (event: MouseEvent<HTMLAnchorElement>) => {
    onClick?.(event);
    if (!shouldNavigateInApp(event)) return;

    const destination = new URL(event.currentTarget.href);
    if (!isSameOriginUrl(destination)) return;

    const isHashOnlyNavigation =
      destination.pathname === window.location.pathname &&
      destination.search === window.location.search &&
      destination.hash !== window.location.hash &&
      destination.hash.length > 0;
    if (isHashOnlyNavigation) return;

    event.preventDefault();
    navigateTo(
      `${destination.pathname}${destination.search}${destination.hash}`,
    );
  };

  return <a href={href} onClick={handleClick} {...props} />;
}

function shouldNavigateInApp(event: MouseEvent<HTMLAnchorElement>) {
  const target = event.currentTarget.getAttribute("target");

  return (
    !event.defaultPrevented &&
    event.button === 0 &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.shiftKey &&
    !event.altKey &&
    (!target || target === "_self") &&
    !event.currentTarget.hasAttribute("download")
  );
}

function isSameOriginUrl(url: URL) {
  return (
    url.origin === window.location.origin &&
    url.protocol === window.location.protocol &&
    url.host === window.location.host
  );
}
