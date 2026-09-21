const assert = require("node:assert/strict");
const test = require("node:test");

const { parseHilCommand } = require("./hil-command.js");
const { dispatchMarker } = require("./hil-find-run.js");
const { evaluateHilRunResults } = require("./hil-gate.js");
const { allowedChips, resolveMatrix } = require("./hil-matrix.js");
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
