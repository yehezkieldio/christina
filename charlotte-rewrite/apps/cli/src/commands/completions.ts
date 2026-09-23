import type { Command } from "commander";

const SHELLS = ["bash", "zsh", "fish", "powershell"] as const;
type Shell = (typeof SHELLS)[number];

const TOP_LEVEL_COMMANDS = [
  "config",
  "profile",
  "stats",
  "sessions",
  "completions",
] as const;

/**
 * Static completion scripts, not `clap_complete`'s dynamically-generated
 * ones: Commander has no built-in completion generator, and the top-level
 * command set is small and stable enough that hand-written templates are
 * simpler than adding a completion-generation dependency for it.
 */
const generateScript = (shell: Shell): string => {
  const words = TOP_LEVEL_COMMANDS.join(" ");
  switch (shell) {
    case "bash": {
      return `_charlotte_completions() {\n  COMPREPLY=($(compgen -W "${words}" -- "\${COMP_WORDS[COMP_CWORD]}"))\n}\ncomplete -F _charlotte_completions charlotte\n`;
    }
    case "zsh": {
      return `#compdef charlotte\n_arguments '1: :(${words})'\n`;
    }
    case "fish": {
      return (
        TOP_LEVEL_COMMANDS.map(
          (word) =>
            `complete -c charlotte -n "__fish_use_subcommand" -a "${word}"`
        ).join("\n") + "\n"
      );
    }
    case "powershell": {
      return `Register-ArgumentCompleter -Native -CommandName charlotte -ScriptBlock {\n  param($wordToComplete)\n  @(${TOP_LEVEL_COMMANDS.map((word) => `'${word}'`).join(", ")}) | Where-Object { $_ -like "$wordToComplete*" }\n}\n`;
    }
    default: {
      return shell satisfies never;
    }
  }
};

export const registerCompletionsCommand = (program: Command): void => {
  program
    .command("completions <shell>")
    .description(`Generate shell completions (${SHELLS.join("|")})`)
    .action((shell: string) => {
      if (!(SHELLS as readonly string[]).includes(shell)) {
        throw new Error(
          `Unsupported shell '${shell}'. Expected one of: ${SHELLS.join(", ")}`
        );
      }
      process.stdout.write(generateScript(shell as Shell));
    });
};
