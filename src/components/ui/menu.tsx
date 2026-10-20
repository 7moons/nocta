import * as Menu from "@radix-ui/react-dropdown-menu";
import { ChevronRight } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";

/**
 * Menus do not block the rest of the page: a dialog opened from a menu item would otherwise
 * inherit the menu's lock and leave the whole app unclickable after it closes.
 */
export function MenuRoot(props: ComponentProps<typeof Menu.Root>) {
  return <Menu.Root modal={false} {...props} />;
}
export const MenuTrigger = Menu.Trigger;
export const MenuSub = Menu.Sub;

export function MenuContent({
  children,
  ...props
}: ComponentProps<typeof Menu.Content>) {
  return (
    <Menu.Portal>
      <Menu.Content className="menu" sideOffset={4} align="start" {...props}>
        {children}
      </Menu.Content>
    </Menu.Portal>
  );
}

export function MenuItem({
  icon,
  danger,
  children,
  ...props
}: ComponentProps<typeof Menu.Item> & { icon?: ReactNode; danger?: boolean }) {
  return (
    <Menu.Item className={`menu-item ${danger ? "danger" : ""}`} {...props}>
      {icon}
      <span>{children}</span>
    </Menu.Item>
  );
}

/** A color sample inside a menu, centered in its cell. */
export function MenuSwatch({
  children,
  ...props
}: ComponentProps<typeof Menu.Item>) {
  return (
    <Menu.Item className="menu-swatch" {...props}>
      {children}
    </Menu.Item>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return <Menu.Label className="menu-label">{children}</Menu.Label>;
}

export function MenuSubTrigger({
  icon,
  children,
}: {
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Menu.SubTrigger className="menu-item">
      {icon}
      <span>{children}</span>
      <ChevronRight size={13} className="menu-chevron" />
    </Menu.SubTrigger>
  );
}

export function MenuSubContent({ children }: { children: ReactNode }) {
  return (
    <Menu.Portal>
      <Menu.SubContent className="menu" sideOffset={4} alignOffset={-4}>
        {children}
      </Menu.SubContent>
    </Menu.Portal>
  );
}

export function MenuSeparator() {
  return <Menu.Separator className="menu-separator" />;
}
