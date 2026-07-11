export const LAB_INTERACT_SUMMARY_LIMITS = Object.freeze({
  detailedAliases: 40,
  detailedSubjects: 24,
});

export function boundedSummary(values, maximum) {
  const source = Array.isArray(values) ? values : [];
  return {
    count: source.length,
    details: source.slice(0, maximum),
    truncated: source.length > maximum,
  };
}
