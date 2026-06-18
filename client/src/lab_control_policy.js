export function createLabControlPolicy({ labClient = null, metadata = null } = {}) {
  return {
    kind: "lab",
    labClient,
    metadata,
    canIssueAs(_playerId) {
      return metadata?.role === "operator";
    },
    destroy() {},
  };
}

export function createDefaultControlPolicy() {
  return {
    kind: "match",
    canIssueAs() {
      return false;
    },
    destroy() {},
  };
}
