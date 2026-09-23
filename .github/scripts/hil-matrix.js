const config = require("../hil-targets.json");

const allowedChips = Array.from(new Set(config.targets.map((target) => target.soc)));

function parseRequestedChips(value) {
  const requested = String(value || "")
    .trim()
    .toLowerCase()
    .split(/[\s,]+/)
    .filter(Boolean);

  if (requested.length === 1 && requested[0] === "all") {
    return allowedChips;
  }

  const chips = Array.from(new Set(requested));
  const unknown = chips.filter((chip) => !allowedChips.includes(chip));
  if (unknown.length) {
    throw new Error(
      `Unsupported HIL chip(s): ${unknown.join(", ")}. Allowed chips: ${allowedChips.join(", ")}`,
    );
  }
  if (!chips.length) {
    throw new Error("At least one HIL chip must be selected");
  }
  return chips;
}

function resolveMatrix(value) {
  const chips = parseRequestedChips(value);
  return {
    chips,
    matrix: {
      target: config.targets.filter((target) => chips.includes(target.soc)),
    },
  };
}

module.exports = { allowedChips, parseRequestedChips, resolveMatrix };
