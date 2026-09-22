import { z } from "zod";

/**
 * Written clamp policy (resolves the open decision in
 * `03-config-and-profiles.md`). Apply it once here; do not re-decide a
 * field's bound at the point of definition.
 *
 * A numeric field gets a hard clamp only when a value outside the bound is
 * not just unwise but impossible to honor: the upstream API rejects it
 * (temperature), or the field directly bounds a shared, concurrent resource
 * that protects the process or the provider from overload (concurrency,
 * partial-failure rate). Every other numeric field is a user-owned budget,
 * token limits, history depth, message length, and gets a lower-bound
 * sanity check only. The user pays for exceeding a budget field, so
 * Charlotte does not second-guess that choice. This mirrors commit
 * `9dcf41c`, which stopped clamping token limits for the same reason.
 */
export function clampedNumber(min: number, max: number) {
  return z.number().min(min).max(max);
}

export function budgetInt(min = 1) {
  return z.number().int().min(min);
}
