#!/usr/bin/env bun
import { optional } from "@charlotte/providers";
import { printError } from "@charlotte/ui";
import {
  constant,
  map,
  message,
  multiple,
  object,
  optional as optionalParser,
  option,
  or,
  string,
} from "@optique/core";
import type { InferValue } from "@optique/core";
import { run } from "@optique/run";

import packageJson from "../package.json" with { type: "json" };
import { configParser, runConfigAction } from "./commands/config";
import { runGenerate } from "./commands/generate";
import { profileParser, runProfileAction } from "./commands/profile";
import {
  runSessionsAction,
  runStatsAction,
  sessionsParser,
  statsParser,
} from "./commands/stats";

/**
 * `-v`/`--verbose` is declared for parity with Christina's `ArgAction::Count`
 * flag (`christina/src/cli/mod.rs`), but nothing downstream reads its value
 * — matches the pre-migration `commander` CLI, which had the same gap.
 */
const generateParser = object({
  context: optionalParser(option("-c", "--context", string())),
  dryRun: option("--dry-run", {
    description: message`Generate commit message without creating the commit (preview mode)`,
  }),
  group: constant("generate"),
  trace: option("--trace", {
    description: message`Enable full pipeline tracing with detailed telemetry output`,
  }),
  verbosity: map(multiple(option("-v", "--verbose")), (flags) => flags.length),
  yes: option("--yes", {
    description: message`Skip interactive confirmations (non-interactive mode)`,
  }),
});

const parser = or(
  configParser,
  profileParser,
  statsParser,
  sessionsParser,
  generateParser
);

type CliResult = InferValue<typeof parser>;

const dispatch = async (result: CliResult): Promise<void> => {
  switch (result.group) {
    case "config": {
      await runConfigAction(result.action);
      return;
    }
    case "profile": {
      await runProfileAction(result.action);
      return;
    }
    case "stats": {
      await runStatsAction();
      return;
    }
    case "sessions": {
      await runSessionsAction(result.action);
      return;
    }
    case "generate": {
      await runGenerate({
        dryRun: result.dryRun,
        trace: result.trace,
        yes: result.yes,
        ...optional("context", result.context),
      });
      return;
    }
    default: {
      result satisfies never;
    }
  }
};

try {
  const result = run(parser, {
    completion: "both",
    description: message`Automated Conventional Commit Generator Powered By LLMs`,
    help: "both",
    programName: "charlotte",
    version: packageJson.version,
  });
  await dispatch(result);
} catch (error) {
  printError(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
