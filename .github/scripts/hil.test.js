const assert = require("node:assert/strict");
const test = require("node:test");

const { parseHilCommand } = require("./hil-command.js");
const { dispatchMarker } = require("./hil-find-run.js");
const { evaluateHilRunResults } = require("./hil-gate.js");
const { allowedChips, resolveMatrix } = require("./hil-matrix.js");
const {
  STATUS_MARKER,
  applyStatus,
  referencesRun,
  runConclusion,
  statusText,
  updateRunStatus,
} = require("./hil-status.js");
const config = require("../hil-targets.json");
const {
  extractTrustedList,
  parseTrustCommand,
  revokeTrusted,
  upsertTrusted,
} = require("./hil-trust.js");

const commandCases = [
  ["/hil quick", { kind: "quick", chips: "esp32c6 esp32s3", suite: "standard" }],
  ["/hil full", { kind: "full", chips: "all", suite: "full" }],
  ["/hil sdm", { kind: "sdm", chips: "esp32c6", suite: "sdm" }],
  ["/hil esp32c3, ESP32S3", { kind: "chips", chips: "esp32c3 esp32s3", suite: "standard" }],
];
for (const [input, expected] of commandCases) {
  test(`parses ${input}`, () => {
    const actual = parseHilCommand(input);
    assert.equal(actual.error, "");
    assert.equal(actual.kind, expected.kind);
    assert.equal(actual.chips, expected.chips);
    assert.equal(actual.suite, expected.suite);
  });
}

test("uses only the first comment line as the command", () => {
  assert.equal(parseHilCommand("/hil quick\nplease run this").kind, "quick");
});

test("rejects unknown chips", () => {
  assert.match(parseHilCommand("/hil esp8266").error, /Unsupported/);
});

test("resolves all configured targets and rejects invalid selections", () => {
  const all = resolveMatrix("all");
  assert.deepEqual(all.chips, allowedChips);
  assert.equal(all.matrix.target.length, config.targets.length);
  assert.throws(() => resolveMatrix("esp32c3 nope"), /Unsupported/);
});

test("resolves every port for a selected chip", () => {
  const selected = resolveMatrix("esp32c6").matrix.target;
  assert.deepEqual(selected.map((target) => target.port), ["usb", "uart"]);
  assert.equal(selected[1].extraArgs, "--extended");
});

test("trust entries round-trip and can be revoked", () => {
  const trusted = upsertTrusted("", "Contributor");
  assert.deepEqual(extractTrustedList(trusted.body), ["contributor"]);
  assert.deepEqual(extractTrustedList(revokeTrusted(trusted.body, "CONTRIBUTOR").body), []);
});

test("trust commands require one GitHub login", () => {
  assert.deepEqual(parseTrustCommand("/trust @Some-One", "trust"), {
    login: "some-one",
    error: "",
  });
  assert.match(parseTrustCommand("/trust some-one", "trust").error, /Usage/);
});

test("dispatch markers require a unique identifier", () => {
  assert.equal(dispatchMarker("12-1"), "[dispatch-id: 12-1]");
  assert.throws(() => dispatchMarker(""), /non-empty/);
});

test("merge queue and slash commands use different failure allowances", () => {
  const results = [{ kind: "passed" }, { kind: "failed" }];
  assert.equal(evaluateHilRunResults(results, 6).pass, true);
  assert.equal(evaluateHilRunResults(results, 0).pass, false);
});

test("run conclusion prefers cancelled over failure and ignores skipped jobs", () => {
  assert.equal(runConclusion(["success", "skipped"]), "success");
  assert.equal(runConclusion(["success", "failure"]), "failure");
  assert.equal(runConclusion(["failure", "cancelled"]), "cancelled");
});

test("status text reports re-run attempts", () => {
  assert.match(statusText("success", 1), /succeeded\.$/);
  assert.match(statusText("failure", 2), /failed on re-run \(attempt 2\) \(failure\)/);
  assert.match(statusText("cancelled", 3), /cancelled on re-run \(attempt 3\)/);
});

test("status updates replace the previous status block", () => {
  const first = applyStatus("Triggered HIL.\n", statusText("failure", 1));
  const second = applyStatus(first, statusText("success", 2));
  assert.equal(second.split(STATUS_MARKER).length, 2);
  assert.match(second, /^Triggered HIL\.\n\n<!-- HIL_STATUS -->\n.*succeeded on re-run/);
  assert.doesNotMatch(second, /failed/);
});

test("run references match the exact run id", () => {
  const url = "https://github.com/o/r/actions/runs/123";
  assert.equal(referencesRun(`Run: ${url}`, url), true);
  assert.equal(referencesRun(`Run: ${url}/attempts/2`, url), true);
  assert.equal(referencesRun(`Run: ${url}4`, url), false);
  assert.equal(referencesRun(`Run: ${url}4 and ${url}`, url), true);
});

function fakeGithub(comments) {
  const calls = { created: [], updated: [] };
  const github = {
    paginate: async () => comments,
    rest: {
      issues: {
        listComments: {},
        createComment: async (args) => calls.created.push(args),
        updateComment: async (args) => calls.updated.push(args),
      },
    },
  };
  return { github, calls };
}

const statusContext = {
  repo: { owner: "o", repo: "r" },
  runId: 123,
  serverUrl: "https://github.com",
};
const quietCore = { info: () => {} };

test("status update edits the dispatcher comment for the run", async () => {
  const { github, calls } = fakeGithub([
    { id: 1, user: { login: "github-actions[bot]" }, body: "Run: https://github.com/o/r/actions/runs/1234" },
    { id: 2, user: { login: "someone" }, body: "Run: https://github.com/o/r/actions/runs/123" },
    { id: 3, user: { login: "github-actions[bot]" }, body: "Run: https://github.com/o/r/actions/runs/123" },
  ]);
  await updateRunStatus({
    github, context: statusContext, core: quietCore, pr: 7, conclusion: "failure", attempt: 1,
  });
  assert.equal(calls.created.length, 0);
  assert.equal(calls.updated.length, 1);
  assert.equal(calls.updated[0].comment_id, 3);
  assert.match(calls.updated[0].body, /HIL run failed \(failure\)/);
});

test("status update creates a comment when the dispatcher comment is missing", async () => {
  const { github, calls } = fakeGithub([]);
  await updateRunStatus({
    github, context: statusContext, core: quietCore, pr: 7, conclusion: "success", attempt: 1,
  });
  assert.equal(calls.updated.length, 0);
  assert.equal(calls.created.length, 1);
  assert.equal(calls.created[0].issue_number, 7);
  assert.match(calls.created[0].body, /actions\/runs\/123\n\n<!-- HIL_STATUS -->\n.*succeeded/);
});

test("status update rejects invalid pull request numbers", async () => {
  const { github } = fakeGithub([]);
  await assert.rejects(
    updateRunStatus({ github, context: statusContext, core: quietCore, pr: NaN, conclusion: "success" }),
    /Invalid pull request number/,
  );
});
