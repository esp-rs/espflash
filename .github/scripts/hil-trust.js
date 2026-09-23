const TRUST_HEADER = "### [HIL trust list]";
const JSON_BEGIN = "<!-- HIL_TRUST_JSON";
const JSON_END = "HIL_TRUST_JSON -->";

function parseTrustCommand(body, command) {
  const text = String(body || "").trim();
  const match = text.match(new RegExp(`^\\/${command}\\s+@([A-Za-z0-9-]+)\\s*$`, "i"));
  if (!match) return { login: "", error: `Usage: /${command} @<login>` };
  return { login: match[1].toLowerCase(), error: "" };
}

function extractTrustedList(existingBody) {
  const match = String(existingBody || "").match(
    /<!--\s*HIL_TRUST_JSON\s*([\s\S]*?)\s*HIL_TRUST_JSON\s*-->/,
  );
  if (!match || !match[1]) return [];
  try {
    const data = JSON.parse(match[1]);
    return Array.isArray(data.trusted)
      ? data.trusted.map((login) => String(login).toLowerCase())
      : [];
  } catch {
    return [];
  }
}

function renderTrustBody(logins) {
  const trusted = Array.from(
    new Set(logins.map((login) => String(login).toLowerCase())),
  ).sort();
  const pretty = trusted.length
    ? trusted.map((login) => `- @${login}`).join("\n")
    : "_None yet_";
  return {
    trusted,
    body: `${TRUST_HEADER}\n${JSON_BEGIN}\n${JSON.stringify({ trusted }, null, 2)}\n${JSON_END}\n\n<details>\n<summary>Trusted users for this PR</summary>\n\n${pretty}\n\n</details>`,
  };
}

function upsertTrusted(existingBody, login) {
  return renderTrustBody([...extractTrustedList(existingBody), login]);
}

function revokeTrusted(existingBody, login) {
  return renderTrustBody(
    extractTrustedList(existingBody).filter(
      (entry) => entry !== String(login || "").toLowerCase(),
    ),
  );
}

module.exports = {
  TRUST_HEADER,
  parseTrustCommand,
  extractTrustedList,
  upsertTrusted,
  revokeTrusted,
};
