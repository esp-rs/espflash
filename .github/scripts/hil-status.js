function statusSuffix(kind, conclusion) {
  if (!conclusion) return `\n\n**Status:** ${kind} did not finish within the reporting window.`;
  if (conclusion === "success") return `\n\n**Status:** ✅ ${kind} succeeded.`;
  if (conclusion === "cancelled") return `\n\n**Status:** ⚠️ ${kind} was cancelled.`;
  return `\n\n**Status:** ❌ ${kind} failed (${conclusion}).`;
}

async function pollRun({
  github,
  context,
  runId,
  commentId,
  kind,
  maxPolls = 180,
  pollIntervalMs = 15000,
}) {
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  let conclusion = null;

  for (let attempt = 0; attempt < maxPolls; attempt++) {
    await delay(pollIntervalMs);
    const { data } = await github.rest.actions.getWorkflowRun({
      ...context.repo,
      run_id: runId,
    });
    if (data.status === "completed") {
      conclusion = data.conclusion;
      break;
    }
  }

  const comment = await github.rest.issues.getComment({
    ...context.repo,
    comment_id: commentId,
  });
  await github.rest.issues.updateComment({
    ...context.repo,
    comment_id: commentId,
    body: `${comment.data.body}${statusSuffix(kind, conclusion)}`,
  });
}

module.exports = { pollRun, statusSuffix };
