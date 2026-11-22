import React from "react";

// The brand name is the same in every language.
const BRAND_NAME = "Dispatch";

/** The Dispatch app icon (branding/dispatch-icon.svg): a crossing signal. */
export const DispatchIcon: React.FC<{ size?: number; className?: string }> = ({
  size = 36,
  className,
}) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 100 100"
    className={className}
    aria-hidden="true"
    xmlns="http://www.w3.org/2000/svg"
  >
    <rect width="100" height="100" rx="22" fill="#0E1A24" />
    <rect x="48" y="22" width="4" height="62" rx="1.5" fill="#A9BCCB" />
    <rect x="38" y="82" width="24" height="5" rx="2" fill="#A9BCCB" />
    <path
      d="M32 12 L68 32 M32 32 L68 12"
      fill="none"
      stroke="#E8EFF4"
      strokeWidth="6"
      strokeLinecap="round"
    />
    <rect x="28" y="44" width="44" height="4" rx="2" fill="#A9BCCB" />
    <circle cx="32" cy="55" r="6.5" fill="#2A4152" />
    <circle cx="68" cy="55" r="6.5" fill="#7DD3FC" />
    <path
      d="M75.7 48.6 A10 10 0 0 1 75.7 61.4"
      fill="none"
      stroke="#7DD3FC"
      strokeWidth="4"
      strokeLinecap="round"
    />
    <path
      d="M81 44.1 A17 17 0 0 1 81 65.9"
      fill="none"
      stroke="#7DD3FC"
      strokeWidth="4"
      strokeLinecap="round"
      opacity="0.6"
    />
  </svg>
);

/** Icon plus the "Dispatch" wordmark in Space Grotesk 700. */
export const DispatchWordmark: React.FC<{
  /** Pixel size of the icon; `null` shows the word alone. */
  iconSize?: number | null;
  textSize?: number;
  className?: string;
}> = ({ iconSize = 36, textSize = 24, className = "" }) => (
  <div
    className={`flex items-center gap-2.5 ${className}`}
    role="img"
    aria-label={BRAND_NAME}
  >
    {iconSize !== null && <DispatchIcon size={iconSize} className="shrink-0" />}
    <span
      className="font-display font-bold leading-none tracking-[-0.3px] text-text"
      style={{ fontSize: textSize }}
      aria-hidden="true"
    >
      {BRAND_NAME}
    </span>
  </div>
);
