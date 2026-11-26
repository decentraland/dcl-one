import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { describe, expect, it, beforeEach, afterEach } from "vitest";

import { redact, secretsOf } from "../../../../rendering/bevy-explorer/source/local-harness/web-driver/driver.mjs";
import { createRedactor, identity, owner } from "./scene-world.mts";

describe("when scene-world prints what a sign-in line touched", () => {
  let dir: string;
  let line: string;
  let key: string;
  let redactor: ReturnType<typeof createRedactor>;

  beforeEach(async () => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), "scene-world-"));
    const kept = await owner(path.join(dir, "owner.json"));
    key = kept.ephemeralKey;
    line = `/hud_login ${identity(kept)} --name SceneOwner --look 1 --realm ${kept.address}.dcl.eth --position 0,0`;
    redactor = createRedactor(redact, secretsOf);
    redactor.add(line);
  });

  afterEach(() => {
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it("should mask the identity in an error that quotes the line", () => {
    const printed = redactor.clean(`\`${line}\` did not finish within 120000 ms`);
    expect(printed).not.toContain(identity(JSON.parse(fs.readFileSync(path.join(dir, "owner.json"), "utf8"))));
    expect(printed).toContain("<redacted>");
  });

  it("should never print the owner's ephemeral key", () => {
    expect(redactor.clean(line)).not.toContain(key.slice(2));
  });

  it("should leave a line without an identity as it is", () => {
    redactor.add("/media_state");
    expect(redactor.clean("/media_state answered")).toBe("/media_state answered");
  });
});
