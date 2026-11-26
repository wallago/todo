import { type ReactNode, useEffect } from "react";
import { TABS } from "../route";

/** A 24 px line icon from SVG path data. */
function Icon({ d }: { d: string }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      width="24"
      height="24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={d} />
    </svg>
  );
}

/** Path data of the icons, by name. */
const ICONS = {
  due: "M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2zM4 10h16M8 2v4M16 2v4M9 15l2 2 4-4",
  pinned: "M12 17v5M9 3h6l-1 7 4 4v3H6v-3l4-4z",
  topics: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
  done: "M20 6 9 17l-5-5",
} as const;

/** The gear at the top left that opens the drawer. */
export function GearButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="icon" aria-label="Menu" onClick={onClick}>
      <Icon d={ICONS.gear} />
    </button>
  );
}

/** The main pages, at the bottom of the screen. */
export function BottomBar({ active }: { active: number }) {
  return (
    <nav className="bottom-bar" aria-label="Pages">
      {TABS.map(({ page, href, label }, i) => (
        <a key={href} href={href} className={i === active ? "active" : ""}>
          <Icon d={ICONS[page]} />
          {label}
        </a>
      ))}
    </nav>
  );
}

/** A row of the drawer: a link, or a button when it has no href. */
function Item({
  icon,
  href,
  onClick,
  children,
}: {
  icon: keyof typeof ICONS;
  href?: string;
  onClick: () => void;
  children: ReactNode;
}) {
  const body = (
    <>
      <Icon d={ICONS[icon]} />
      <span>{children}</span>
    </>
  );
  return (
    <li>
      {href ? (
        <a className="drawer-item" href={href} onClick={onClick}>
          {body}
        </a>
      ) : (
        <button type="button" className="drawer-item" onClick={onClick}>
          {body}
        </button>
      )}
    </li>
  );
}

/** The side drawer: secondary pages and Settings. Esc or a tap outside closes it. */
export function Drawer({
  status,
  onSettings,
  onClose,
}: {
  status: string;
  onSettings: () => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <>
      <button type="button" className="drawer-backdrop" aria-label="Close menu" onClick={onClose} />
      <aside className="drawer" aria-label="Menu">
        <p className="drawer-title">Todo</p>
        <p className="drawer-status">{status || "synced"}</p>
        <ul>
          <Item icon="done" href="#/done-today" onClick={onClose}>
            Done today
          </Item>
          {/* History goes here. */}
          <Item
            icon="gear"
            onClick={() => {
              onClose();
              onSettings();
            }}
          >
            Settings
          </Item>
        </ul>
      </aside>
    </>
  );
}
