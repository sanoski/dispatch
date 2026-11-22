// Dispatch: the little crossing signal in the recording pill, in place of
// Handy's pulsing dot (Joshua's idea, 2026-11-22). Its right lamp glows
// with your voice, and the pill's voice bars sit right against it, so they
// read as sound coming off the lamp, like the waves in the splash art. Kept
// a little see-through on purpose (Joshua: "Just a little transparent").
import React from "react";
import "./SignalMark.css";

/** `levels` are the pill's smoothed bar levels, 0 to 1. */
export const SignalMark: React.FC<{ ready: boolean; levels: number[] }> = ({
  ready,
  levels,
}) => {
  const level = ready ? Math.min(1, Math.pow(Math.max(0, ...levels), 0.7)) : 0;
  return (
    <svg
      className={`dsig-mark ${ready ? "ready" : "arming"}`}
      viewBox="0 0 19 30"
      width="17"
      height="27"
      aria-hidden="true"
    >
      <path className="dsig-metal" d="M4.5 1 L13.5 6.2 M4.5 6.2 L13.5 1" />
      <rect className="dsig-fill" x="8.2" y="2" width="1.6" height="22.6" />
      <rect
        className="dsig-fill"
        x="2.5"
        y="9.4"
        width="13"
        height="1.6"
        rx="0.8"
      />
      <rect
        className="dsig-fill"
        x="5.5"
        y="23.8"
        width="7"
        height="1.8"
        rx="0.9"
      />
      <circle className="dsig-dark" cx="4.5" cy="15" r="3.2" />
      <circle
        className="dsig-halo"
        cx="14.5"
        cy="15"
        r={3.4 + 2.4 * level}
        style={{ opacity: ready ? 0.12 + 0.35 * level : 0 }}
      />
      <circle
        className="dsig-lamp"
        cx="14.5"
        cy="15"
        r="3.2"
        style={{ opacity: ready ? 0.65 + 0.35 * level : 0.45 }}
      />
    </svg>
  );
};
