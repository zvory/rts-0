import {
  INTERACT_COMMAND_REGISTRY,
  INTERACT_COMMANDS,
} from "./command_registry.ts";

export const INTERACT_COMMAND_HELP = Object.freeze(Object.fromEntries(
  INTERACT_COMMANDS.map((command) => [command, INTERACT_COMMAND_REGISTRY[command].help]),
));

export function commandHelp(command: string) {
  return INTERACT_COMMAND_REGISTRY[command]?.help || null;
}

export function helpCatalog() {
  return INTERACT_COMMANDS.map((command) => ({
    command,
    summary: INTERACT_COMMAND_REGISTRY[command].help.summary,
  }));
}
