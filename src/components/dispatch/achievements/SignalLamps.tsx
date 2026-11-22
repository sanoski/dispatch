// Dispatch achievements: the crossing-signal badge. Lit lamps mean unlocked;
// `flashing` alternates them like a real crossing (the unlock pop-up).
import React from "react";

export type LampState = "lit" | "half" | "dark" | "secret";

interface SignalLampsProps {
  state: LampState;
  size?: "card" | "tier" | "popup";
  flashing?: boolean;
  label?: string;
}

const DIM = "var(--color-line-strong)";
const SECRET = "color-mix(in srgb, var(--color-line) 70%, transparent)";
const ACCENT = "var(--color-logo-primary)";

export const SignalLamps: React.FC<SignalLampsProps> = ({
  state,
  size = "card",
  flashing = false,
  label,
}) => {
  const lampA =
    state === "lit" || state === "half"
      ? ACCENT
      : state === "secret"
        ? SECRET
        : DIM;
  const lampB = state === "lit" ? ACCENT : state === "secret" ? SECRET : DIM;
  const cross = state === "lit" ? "var(--color-ink-2)" : "var(--color-ink-3)";
  const glow =
    state === "lit"
      ? "drop-shadow-[0_0_6px_color-mix(in_srgb,var(--color-logo-primary),transparent_45%)]"
      : "";
  const a11y = label
    ? { role: "img", "aria-label": label }
    : { "aria-hidden": true as const };

  if (size === "popup") {
    return (
      <svg
        width="64"
        height="56"
        viewBox="0 0 64 56"
        className={`shrink-0 ${glow}`}
        {...a11y}
      >
        <path
          d="M20 4l24 13M20 17l24-13"
          stroke={cross}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <rect
          x="30"
          y="17"
          width="4"
          height="34"
          rx="1.5"
          fill="var(--color-ink-3)"
        />
        <rect x="8" y="24" width="48" height="4" rx="2" fill={DIM} />
        <circle
          className={flashing ? "dispatch-lamp-a" : undefined}
          cx="17"
          cy="38"
          r="9"
          fill={lampA}
        />
        <circle
          className={flashing ? "dispatch-lamp-b" : undefined}
          cx="47"
          cy="38"
          r="9"
          fill={lampB}
        />
      </svg>
    );
  }

  if (size === "tier") {
    return (
      <svg
        width="30"
        height="30"
        viewBox="0 0 30 30"
        className="shrink-0"
        {...a11y}
      >
        <path
          d="M9 3l12 7M9 10l12-7"
          stroke="var(--color-ink-3)"
          strokeWidth="2"
          strokeLinecap="round"
        />
        <rect x="4" y="14" width="22" height="2.5" rx="1.2" fill={DIM} />
        <circle cx="9" cy="22" r="5" fill={lampA} />
        <circle cx="21" cy="22" r="5" fill={lampB} />
      </svg>
    );
  }

  return (
    <svg width="46" height="40" viewBox="0 0 46 40" className={glow} {...a11y}>
      <path
        d="M15 3l16 9M15 12l16-9"
        stroke={cross}
        strokeWidth="2.5"
        strokeLinecap="round"
      />
      <rect x="6" y="17" width="34" height="3" rx="1.5" fill={DIM} />
      <circle cx="13" cy="29" r="7" fill={lampA} />
      <circle cx="33" cy="29" r="7" fill={lampB} />
    </svg>
  );
};

/** The sidebar icon: a crossbuck over two lamps, drawn in currentColor. */
export const AchievementsIcon: React.FC<{
  width?: number | string;
  height?: number | string;
  className?: string;
}> = ({ width = 24, height = 24, className }) => (
  <svg
    width={width}
    height={height}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    aria-hidden="true"
  >
    <path d="M8 4l8 6M8 10l8-6" />
    <path d="M5 13h14" />
    <circle cx="8" cy="17.5" r="2.5" />
    <circle cx="16" cy="17.5" r="2.5" />
  </svg>
);
