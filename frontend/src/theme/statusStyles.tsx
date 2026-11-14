import type { ReactNode } from "react";
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  ExclamationCircleOutlined,
  MinusCircleOutlined,
  QuestionCircleOutlined,
  WarningOutlined,
} from "@ant-design/icons";

/**
 * Health status of a monitor or a timeline bucket. Mirrors the statuses the
 * backend emits (`up`, `warning`, `down`, `error`, `indeterminate`, `no_data`).
 */
export type Status =
  | "up"
  | "warning"
  | "down"
  | "error"
  | "indeterminate"
  | "no_data";

export interface StatusStyle {
  /** Base fill colour, shared by timeline bars, tags, cards and tiles. */
  color: string;
  /**
   * Background of the status `Tag`. Same hex as `color` (the state keeps its
   * identity). IMPORTANT: the `Tag` must be rendered with `variant="solid"`, so
   * antd paints this hex as the actual `background-color`. With antd's default
   * `filled` variant a custom non-preset colour is treated as *text* colour and
   * the background becomes a light tint (`hsl.l = 0.95`) — which would break the
   * ratios below (e.g. `no_data` tint `#f1f2f3` + white = 1.12:1).
   */
  tagBg: string;
  /**
   * Accessible text colour for the status `Tag`, verified against the *rendered*
   * background (`tagBg`, i.e. `variant="solid"`): every pair is >= 4.5:1
   * (WCAG AA, normal text) and, being a fixed hex, identical in light and dark
   * themes. Measured ratios are annotated per status.
   */
  tagText: string;
  /**
   * Non-chromatic channel #1: a CSS `background-image` built from a
   * `repeating-linear-gradient`. The angle/period differ per status so bars
   * stay distinguishable in greyscale.
   */
  pattern: string;
  /**
   * Non-chromatic channel #2: the top edge of a bar. Thickness AND style differ
   * per status, so the state stays readable even on very small bars (where the
   * hatch pattern is not perceptible).
   */
  borderTop: string;
  /** Human readable label used in tags, legends and accessible names. */
  label: string;
  /** Icon used in the detail tooltip / cards (already coloured, size 24). */
  icon: ReactNode;
}

const ICON_SIZE = 24;

/** Near-black used as the accessible text colour on light status surfaces. */
const INK = "#111827";
/** White used as the accessible text colour on dark status surfaces. */
const PAPER = "#ffffff";

/**
 * Single source of truth for the status palette. Every consumer (timeline,
 * MonitorCard, Dashboard) reads from here instead of hard-coding hex values.
 *
 * `tagBg`/`tagText` pairs are contrast-checked on the *rendered* solid surface
 * (WCAG AA, >= 4.5:1), identical in light and dark theme:
 * `up` 7.79 · `warning` 8.26 · `down` 4.71 · `error` 6.33 ·
 * `indeterminate` 6.94 · `no_data` 4.83.
 */
export const STATUS_STYLES: Record<Status, StatusStyle> = {
  up: {
    color: "#22c55e",
    // On solid #22c55e: white only reaches 2.28:1, ink reaches 7.79:1.
    tagBg: "#22c55e",
    tagText: INK,
    // Solid, no top edge: the baseline "healthy" look.
    pattern: "none",
    borderTop: "none",
    label: "UP",
    icon: <CheckCircleOutlined aria-hidden style={{ color: "#22c55e", fontSize: ICON_SIZE }} />,
  },
  warning: {
    color: "#f59e0b",
    // On solid #f59e0b: white only reaches 2.15:1, ink reaches 8.26:1.
    tagBg: "#f59e0b",
    tagText: INK,
    // Diagonal hatch (45°), medium period.
    pattern:
      "repeating-linear-gradient(45deg, rgba(0,0,0,0.30) 0 3px, rgba(0,0,0,0) 3px 7px)",
    // Thin solid top edge.
    borderTop: "2px solid rgba(0,0,0,0.55)",
    label: "WARNING",
    icon: <WarningOutlined aria-hidden style={{ color: "#f59e0b", fontSize: ICON_SIZE }} />,
  },
  down: {
    color: "#ef4444",
    // On solid #ef4444: white only reaches 3.76:1, ink reaches 4.71:1.
    tagBg: "#ef4444",
    tagText: INK,
    // Horizontal stripes (0°), dense.
    pattern:
      "repeating-linear-gradient(0deg, rgba(0,0,0,0.35) 0 2px, rgba(0,0,0,0) 2px 5px)",
    // Thick solid top edge.
    borderTop: "3px solid rgba(0,0,0,0.55)",
    label: "DOWN",
    icon: <CloseCircleOutlined aria-hidden style={{ color: "#ef4444", fontSize: ICON_SIZE }} />,
  },
  error: {
    color: "#f97316",
    // On solid #f97316: white only reaches 2.80:1, ink reaches 6.33:1.
    tagBg: "#f97316",
    tagText: INK,
    // Vertical stripes (90°).
    pattern:
      "repeating-linear-gradient(90deg, rgba(0,0,0,0.35) 0 2px, rgba(0,0,0,0) 2px 6px)",
    // Thick DOUBLE top edge: distinct from `warning`'s thin solid edge even when
    // the 45°/90° hatch is too fine to see.
    borderTop: "3px double rgba(0,0,0,0.70)",
    label: "ERROR",
    icon: <ExclamationCircleOutlined aria-hidden style={{ color: "#f97316", fontSize: ICON_SIZE }} />,
  },
  indeterminate: {
    color: "#722ed1",
    // On solid #722ed1: ink only reaches 2.56:1, white reaches 6.94:1.
    tagBg: "#722ed1",
    tagText: PAPER,
    // Counter-diagonal hatch (135°) with a light overlay: reads as "unknown",
    // never as a real outage.
    pattern:
      "repeating-linear-gradient(135deg, rgba(255,255,255,0.55) 0 2px, rgba(255,255,255,0) 2px 6px)",
    // Dotted light top edge.
    borderTop: "2px dotted rgba(255,255,255,0.95)",
    label: "INDETERMINATE",
    icon: <QuestionCircleOutlined aria-hidden style={{ color: "#722ed1", fontSize: ICON_SIZE }} />,
  },
  no_data: {
    color: "#6b7280",
    // On solid #6b7280: ink only reaches 3.67:1, white reaches 4.83:1.
    tagBg: "#6b7280",
    tagText: PAPER,
    // Sparse diagonal hatch (-45°), low contrast: "nothing here".
    pattern:
      "repeating-linear-gradient(-45deg, rgba(0,0,0,0.20) 0 2px, rgba(0,0,0,0) 2px 9px)",
    // Hairline dashed top edge.
    borderTop: "1px dashed rgba(0,0,0,0.40)",
    label: "SIN DATOS",
    icon: <MinusCircleOutlined aria-hidden style={{ color: "#6b7280", fontSize: ICON_SIZE }} />,
  },
};

/**
 * Resolve the style for a (possibly unknown) status string, falling back to
 * `no_data` so unmapped statuses never crash the timeline.
 *
 * An own-property check is required (not `status in STATUS_STYLES`): `"toString"
 * in STATUS_STYLES` is `true` via the prototype chain and would return a
 * function instead of a style. `Object.hasOwn` is the idiomatic form but needs
 * `lib: es2022`, which this project does not set, so we call the prototype
 * method directly — same semantics, no tsconfig change.
 */
export function statusStyle(status: string | null | undefined): StatusStyle {
  if (
    typeof status === "string" &&
    Object.prototype.hasOwnProperty.call(STATUS_STYLES, status)
  ) {
    return STATUS_STYLES[status as Status];
  }
  return STATUS_STYLES.no_data;
}
