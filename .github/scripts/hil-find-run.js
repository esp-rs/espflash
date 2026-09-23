function dispatchMarker(distinctId) {
  const id = String(distinctId || "").trim();
  if (!id) throw new Error("A non-empty dispatch identifier is required");
  return `[dispatch-id: ${id}]`;
}

async function findDispatchedRun({
  github,
  context,
  distinctId,
  attempts = 20,
  delayMs = 3000,
}) {
  const marker = dispatchMarker(distinctId);
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  for (let attempt = 1; attempt <= attempts; attempt++) {
    const { data } = await github.rest.actions.listWorkflowRuns({
      ...context.repo,
      workflow_id: "hil.yml",
      event: "workflow_dispatch",
      per_page: 50,
    });
    const run = (data.workflow_runs || []).find((candidate) =>
      String(candidate.display_title || candidate.name || "").includes(marker),
    );
    if (run) return run;
    if (attempt < attempts) await delay(delayMs);
  }
  return null;
}

module.exports = { dispatchMarker, findDispatchedRun };
