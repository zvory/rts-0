export const LAB_SCENARIO_SUBMISSION_CAPABILITY_PATH = "/api/lab-scenarios/submission";

const DEFAULT_CAPABILITY_RETRY_DELAYS_MS = Object.freeze([
  500,
  1500,
  3000,
  5000,
  10000,
  20000,
  30000,
  30000,
  30000,
  30000,
  30000,
  30000,
  30000,
  30000,
]);

export async function fetchLabScenarioSubmissionCapability({
  fetchImpl = globalThis.fetch,
  retryDelaysMs = DEFAULT_CAPABILITY_RETRY_DELAYS_MS,
  sleep = delay,
} = {}) {
  if (typeof fetchImpl !== "function") {
    return failedCapability(
      "Scenario PR submission capability check failed: fetch is unavailable.",
    );
  }

  let failureReason = "Scenario PR submission capability check failed.";
  const attempts = Math.max(1, retryDelaysMs.length + 1);
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    let shouldRetry = true;
    try {
      const response = await fetchImpl(
        LAB_SCENARIO_SUBMISSION_CAPABILITY_PATH,
        { cache: "no-store" },
      );
      if (response?.ok) return await response.json();
      const status = Number(response?.status || 0);
      failureReason = `Scenario PR submission capability check failed (${status || "unknown"}).`;
      shouldRetry = transientCapabilityStatus(status);
    } catch (err) {
      failureReason = `Scenario PR submission capability check failed: ${err?.message || err}`;
    }

    if (!shouldRetry || attempt >= retryDelaysMs.length) break;
    await sleep(Math.max(0, Number(retryDelaysMs[attempt]) || 0));
  }

  return failedCapability(failureReason);
}

function failedCapability(unavailableReason) {
  return {
    available: false,
    unavailableCode: "capabilityCheckFailed",
    unavailableReason,
  };
}

function transientCapabilityStatus(status) {
  return status === 0 || status === 408 || status === 429 || status >= 500;
}

function delay(ms) {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, ms);
  });
}
