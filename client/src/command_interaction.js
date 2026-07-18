/**
 * Owns the shared gameplay-command issue-and-record interaction used by client controls.
 */
export class CommandInteraction {
  constructor({ commandIssuer, clientIntent, selectedEntities = () => [] } = {}) {
    this.commandIssuer = commandIssuer;
    this.clientIntent = clientIntent;
    this.selectedEntities = selectedEntities;
  }

  issueCommand(command, options = {}) {
    const selected = this.selectedEntities?.() || [];
    const result = issueGameplayCommand(this.commandIssuer, command, options);
    this.clientIntent?.recordPlannedCommand?.(command, selected, result);
    return result;
  }
}

function issueGameplayCommand(sender, command, options) {
  if (sender && typeof sender.issueCommand === "function") {
    return sender.issueCommand(command, options);
  }
  if (sender && typeof sender.command === "function" && sender.command.length < 2) {
    return sender.command(command);
  }
  return false;
}
