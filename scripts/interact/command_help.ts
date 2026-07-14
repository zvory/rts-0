import {
  INTERACT_COMMAND_REGISTRY,
  INTERACT_COMMANDS,
  INTERACT_NAMESPACES,
  namespaceCommandKey,
} from "./command_registry.ts";

export const INTERACT_COMMAND_HELP = Object.freeze(Object.fromEntries(
  INTERACT_COMMANDS.map((command) => [command, INTERACT_COMMAND_REGISTRY[command].help]),
));

export function commandHelp(command: string, namespace = "lab") {
  const key = namespaceCommandKey(namespace, command);
  return key ? INTERACT_COMMAND_REGISTRY[key]?.help || null : null;
}

export function helpCatalog(namespace = "lab") {
  return (INTERACT_NAMESPACES[namespace] || []).map((command) => ({
    command,
    summary: commandHelp(command, namespace)?.summary || "",
  }));
}
