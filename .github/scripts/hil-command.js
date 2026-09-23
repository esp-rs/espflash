const { allowedChips } = require("./hil-matrix.js");
const config = require("../hil-targets.json");

const usage = [
  "Usage:",
  "- `/hil quick` — test ESP32-C6 and ESP32-S3",
  "- `/hil full` — run the complete HIL and SDM suites",
  "- `/hil sdm` — run only the ESP32-C6 secure-download-mode suite",
  "- `/hil <chip1> [<chip2> ...]` — test every configured port for selected chips",
  `- Supported chips: ${allowedChips.map((chip) => `\`${chip}\``).join(", ")}`,
].join("\n");

function parseHilCommand(body) {
  const line = String(body || "").trim().split(/\r?\n/, 1)[0].trim();
  const match = line.match(/^\/hil(?:\s+(.*))?$/i);
  if (!match || !match[1] || /^help$/i.test(match[1].trim())) {
    return { kind: "help", error: "", chips: "", suite: "" };
  }

  const argument = match[1].trim().toLowerCase();
  if (argument === "quick") {
    return {
      kind: "quick",
      error: "",
      chips: config.quickChips.join(" "),
      suite: "standard",
    };
  }
  if (argument === "full") {
    return { kind: "full", error: "", chips: "all", suite: "full" };
  }
  if (argument === "sdm") {
    return { kind: "sdm", error: "", chips: "esp32c6", suite: "sdm" };
  }

  const requested = Array.from(
    new Set(argument.split(/[\s,]+/).filter(Boolean)),
  );
  const unknown = requested.filter((chip) => !allowedChips.includes(chip));
  if (!requested.length || unknown.length) {
    return {
      kind: "invalid",
      error: unknown.length
        ? `Unsupported HIL chip(s): ${unknown.join(", ")}`
        : "No HIL chips were selected",
      chips: "",
      suite: "",
    };
  }

  return {
    kind: "chips",
    error: "",
    chips: requested.join(" "),
    suite: "standard",
  };
}

module.exports = { parseHilCommand, usage };
