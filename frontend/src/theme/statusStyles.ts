import { createElement } from "react";
import type { ComponentType, CSSProperties, ReactNode } from "react";
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

/** Decorative icon: `aria-hidden` so it never produces a duplicate announcement. */
function decorativeIcon(
  Component: ComponentType<{ "aria-hidden"?: boolean; style?: CSSProperties }>,
  color: string,
): ReactNode {
  return createElement(Component, {
    "aria-hidden": true,
    style: { color, fontSize: ICON_SIZE },
  });
}

/**
 * Single source of truth for the status palette. Every consumer (timeline,
 * MonitorCard, Dashboard) reads from here instead of hard-coding hex values.
 */
export const STATUS_STYLES: Record<Status, StatusStyle> = {
  up: {
    color: "#22c55e",
    // Solid, no top edge: the baseline "healthy" look.
    pattern: "none",
    borderTop: "none",
    label: "UP",
    icon: decorativeIcon(CheckCircleOutlined, "#22c55e"),
  },
  warning: {
    color: "#f59e0b",
    // Diagonal hatch (45°), medium period.
    pattern:
      "repeating-linear-gradient(45deg, rgba(0,0,0,0.30) 0 3px, rgba(0,0,0,0) 3px 7px)",
    // Thin solid top edge.
    borderTop: "2px solid rgba(0,0,0,0.55)",
    label: "WARNING",
    icon: decorativeIcon(WarningOutlined, "#f59e0b"),
  },
  down: {
    color: "#ef4444",
    // Horizontal stripes (0°), dense.
    pattern:
      "repeating-linear-gradient(0deg, rgba(0,0,0,0.35) 0 2px, rgba(0,0,0,0) 2px 5px)",
    // Thick solid top edge.
    borderTop: "3px solid rgba(0,0,0,0.55)",
    label: "DOWN",
    icon: decorativeIcon(CloseCircleOutlined, "#ef4444"),
  },
  error: {
    color: "#f97316",
    // Vertical stripes (90°).
    pattern:
      "repeating-linear-gradient(90deg, rgba(0,0,0,0.35) 0 2px, rgba(0,0,0,0) 2px 6px)",
    // Thick DOUBLE top edge: distinct from `warning`'s thin solid edge even when
    // the 45°/90° hatch is too fine to see.
    borderTop: "3px double rgba(0,0,0,0.70)",
    label: "ERROR",
    icon: decorativeIcon(ExclamationCircleOutlined, "#f97316"),
  },
  indeterminate: {
    color: "#722ed1",
    // Counter-diagonal hatch (135°) with a light overlay: reads as "unknown",
    // never as a real outage.
    pattern:
      "repeating-linear-gradient(135deg, rgba(255,255,255,0.55) 0 2px, rgba(255,255,255,0) 2px 6px)",
    // Dotted light top edge.
    borderTop: "2px dotted rgba(255,255,255,0.95)",
    label: "INDETERMINATE",
    icon: decorativeIcon(QuestionCircleOutlined, "#722ed1"),
  },
  no_data: {
    color: "#6b7280",
    // Sparse diagonal hatch (-45°), low contrast: "nothing here".
    pattern:
      "repeating-linear-gradient(-45deg, rgba(0,0,0,0.20) 0 2px, rgba(0,0,0,0) 2px 9px)",
    // Hairline dashed top edge.
    borderTop: "1px dashed rgba(0,0,0,0.40)",
    label: "SIN DATOS",
    icon: decorativeIcon(MinusCircleOutlined, "#6b7280"),
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
