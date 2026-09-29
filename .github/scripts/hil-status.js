// Verdict reporting for dispatched HIL runs.
//
// hil.yml's report job calls this when a dispatched run finishes, including
// re-runs and cancelled runs.
const STATUS_MARKER = "<!-- HIL_STATUS -->";

function statusText(conclusion, attempt) {
  const rerun = attempt > 1 ? ` on re-run (attempt ${attempt})` : "";
  if (conclusion === "success") return `**Status:** ✅ HIL run succeeded${rerun}.`;
  if (conclusion === "cancelled") return `**Status:** ⚠️ HIL run was cancelled${rerun}.`;
  return `**Status:** ❌ HIL run failed${rerun} (${conclusion}).`;
}

// Replace any previous status so re-runs update the same block.
function applyStatus(body, text) {
  const [head] = body.split(STATUS_MARKER);
  return `${head.trimEnd()}\n\n${STATUS_MARKER}\n${text}`;
}

// Match the exact run, so run 123 does not match a comment for run 1234.
function referencesRun(body, runUrl) {
  let index = body.indexOf(runUrl);
  while (index !== -1) {
    if (!/\d/.test(body[index + runUrl.length] ?? "")) return true;
    index = body.indexOf(runUrl, index + 1);
  }
  return false;
}

function runConclusion(results) {
  if (results.includes("cancelled")) return "cancelled";
  if (results.includes("failure")) return "failure";
  return "success";
}

async function updateRunStatus({ github, context, core, pr, conclusion, attempt }) {
  if (!Number.isInteger(pr) || pr <= 0) throw new Error(`Invalid pull request number: ${pr}`);
  const { owner, repo } = context.repo;
  const runUrl = `${context.serverUrl}/${owner}/${repo}/actions/runs/${context.runId}`;
  const text = statusText(conclusion, attempt);

  // The dispatcher's confirmation comment is the one carrying this run's URL.
  const comments = await github.paginate(github.rest.issues.listComments, {
    owner,
    repo,
    issue_number: pr,
    per_page: 100,
  });
  const comment = comments.findLast(
    (c) => c.user?.login === "github-actions[bot]" && referencesRun(c.body ?? "", runUrl),
  );

  if (comment) {
    await github.rest.issues.updateComment({
      owner,
      repo,
      comment_id: comment.id,
      body: applyStatus(comment.body, text),
    });
    return;
  }

  // The confirmation comment may be missing if posting it failed or the run
  // URL could not be resolved; report the verdict in a new comment instead.
  core.info(`No bot comment on #${pr} references ${runUrl}; creating one.`);
  await github.rest.issues.createComment({
    owner,
    repo,
    issue_number: pr,
    body: applyStatus(`HIL run for PR #${pr}: ${runUrl}`, text),
  });
}

module.exports = {
  STATUS_MARKER,
  applyStatus,
  referencesRun,
  runConclusion,
  statusText,
  updateRunStatus,
};
