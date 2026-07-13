import {
  LAB_INTERACT_COMMAND_REGISTRY,
  LAB_INTERACT_COMMANDS,
} from "./command_registry.mjs";

export const LAB_INTERACT_COMMAND_HELP = Object.freeze(Object.fromEntries(
  LAB_INTERACT_COMMANDS.map((command) => [command, LAB_INTERACT_COMMAND_REGISTRY[command].help]),
));

export function commandHelp(command) {
  return LAB_INTERACT_COMMAND_REGISTRY[command]?.help || null;
}

export function helpCatalog() {
  return LAB_INTERACT_COMMANDS.map((command) => ({
    command,
    summary: LAB_INTERACT_COMMAND_REGISTRY[command].help.summary,
  }));
}
