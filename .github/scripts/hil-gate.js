// Merge-queue gate for HIL matrix legs.
//
// Passes when at most MAX_FAILED_RUNS executed `HIL | <soc> | <port>` jobs
// failed or were cancelled. Dual-port chips (uart + usb) count as two jobs.
// Skipped jobs are excluded. `HIL (SDM)` is informational and is not gated.
//
// Classification uses the job conclusion rather than a single step: a dead
// runner typically fails "Prepare device" and never reaches "Run all tests".

const MAX_FAILED_RUNS = 6;

function isHilRunMatrixJob(name) {
  // Matches "HIL | …" but not "HIL (SDM) | …".
  return /^HIL \| /i.test(String(name || ""));
}

function classifyMatrixJob(job) {
  const conclusion = job.conclusion;
  if (conclusion === "skipped") {
    return { kind: "skipped" };
  }
  if (conclusion === "success") {
    return { kind: "passed" };
  }
  if (
    conclusion === "failure" ||
    conclusion === "cancelled" ||
    conclusion === null
  ) {
    return { kind: "failed" };
  }

  return {
    error: `job "${job.name}" has unexpected conclusion: ${conclusion}`,
  };
}

function evaluateHilRunResults(classifications) {
  const executed = classifications.filter(
    (c) => c.kind === "passed" || c.kind === "failed",
  );
  const failures = classifications.filter((c) => c.kind === "failed");

  if (executed.length === 0) {
    return { pass: true, executed: 0, failures: 0 };
  }

  const pass = failures.length <= MAX_FAILED_RUNS;
  return { pass, executed: executed.length, failures: failures.length };
}

function isNewerJob(candidate, current) {
  const candidateAttempt = candidate.run_attempt ?? 0;
  const currentAttempt = current.run_attempt ?? 0;
  if (candidateAttempt !== currentAttempt) {
    return candidateAttempt > currentAttempt;
  }
  return (candidate.id ?? 0) > (current.id ?? 0);
}

// Keep the newest attempt of each job name. Re-run failed jobs leaves
// untouched legs on earlier attempts; without this the gate would either
// miss them (filter=latest) or double-count them (filter=all, no dedupe).
function latestJobsByName(jobs) {
  const byName = new Map();
  for (const job of jobs) {
    const name = String(job.name || "");
    const current = byName.get(name);
    if (!current || isNewerJob(job, current)) {
      byName.set(name, job);
    }
  }
  return [...byName.values()];
}

async function listWorkflowRunJobs(github, context) {
  const { owner, repo } = context.repo;
  const run_id = context.runId;
  const jobs = [];
  let page = 1;

  while (true) {
    const { data } = await github.rest.actions.listJobsForWorkflowRun({
      owner,
      repo,
      run_id,
      filter: "all",
      per_page: 100,
      page,
    });

    jobs.push(...(data.jobs || []));
    if (jobs.length >= data.total_count) {
      break;
    }
    page += 1;
  }

  return jobs;
}

async function evaluateHilGate({ github, context, core }) {
  const jobs = await listWorkflowRunJobs(github, context);
  const matrixJobs = latestJobsByName(
    jobs.filter((job) => isHilRunMatrixJob(job.name)),
  );

  if (matrixJobs.length === 0) {
    core.setFailed("HIL gate failed: could not find HIL matrix jobs");
    return;
  }

  const classifications = [];
  for (const job of matrixJobs) {
    const result = classifyMatrixJob(job);
    if (result.error) {
      core.setFailed(`HIL gate failed: ${result.error}`);
      return;
    }
    classifications.push(result);
  }

  const verdict = evaluateHilRunResults(classifications);
  if (verdict.pass) {
    if (verdict.executed > 0) {
      core.info(
        `HIL gate passed (${verdict.failures}/${verdict.executed} executed runners failed)`,
      );
    } else {
      core.info("HIL gate passed (no HIL matrix legs executed tests)");
    }
    return;
  }

  core.setFailed(
    `HIL gate failed: ${verdict.failures}/${verdict.executed} executed runners failed (max ${MAX_FAILED_RUNS})`,
  );
}

module.exports = {
  MAX_FAILED_RUNS,
  isHilRunMatrixJob,
  classifyMatrixJob,
  evaluateHilRunResults,
  latestJobsByName,
  evaluateHilGate,
};
