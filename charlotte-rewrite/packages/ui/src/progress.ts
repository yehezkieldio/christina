import type { PipelineStage } from "@charlotte/schemas";
import type { Spinner } from "./spinner";

/** The progress event Christina's `send_generation_progress` pushes down
 * `generate.rs`'s channel. `stage` is the structural pipeline stage (also
 * used by the `stage_start`/`stage_end` session events); `message` is the
 * free-text spinner line Christina's call sites already pass
 * (`"connecting to provider"`, `"processing diff"`, ...). */
export interface ProgressEvent {
  readonly stage: PipelineStage;
  readonly message: string;
}

export type ProgressListener = (event: ProgressEvent) => void;

/** Per `09-cli-and-ui.md`: "the same event then powers both the live
 * spinner and the persistent record." This only renders the spinner half;
 * writing the event to the session transcript is the caller's job
 * (`@charlotte/session`'s `SessionWriter`), not this package's. */
export function bindSpinnerToProgress(spinner: Spinner): ProgressListener {
  return (event) => spinner.update(event.message);
}
