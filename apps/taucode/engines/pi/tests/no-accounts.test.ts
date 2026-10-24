import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { promisify } from "node:util";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

test("auth storage stays empty across sync, promise and callback APIs", async () => {
  const dir = fs.mkdtempSync(join(tmpdir(), "tau-auth-"));
  process.env.PI_CODING_AGENT_DIR = dir;
  try {
    await import("../no-accounts.mjs");
    for (const name of ["auth.json", "mcp-auth.json"]) {
      const path = join(dir, name);
      for (const input of [path, Buffer.from(path), pathToFileURL(path)]) {
        assert.equal(fs.readFileSync(input, "utf8"), "{}");
        assert.equal(await readFile(input, "utf8"), "{}");
        assert.deepEqual(await fs.promises.readFile(input), Buffer.from("{}"));
        assert.equal(await promisify(fs.readFile)(input, "utf8"), "{}");
        assert.deepEqual(await promisify(fs.readFile)(input), Buffer.from("{}"));
        fs.writeFileSync(input, " {} ");
        await writeFile(input, Buffer.from("{}"));
        await promisify(fs.writeFile)(input, "{}", "utf8");
        await promisify(fs.writeFile)(input, "{}");
        assert.throws(() => fs.writeFileSync(input, '{"key":"secret"}'), /disables accounts/);
        await assert.rejects(writeFile(input, '{"key":"secret"}'), /disables accounts/);
        await assert.rejects(promisify(fs.writeFile)(input, '{"key":"secret"}'), /disables accounts/);
      }
      assert.equal(fs.existsSync(path), false);
    }
    const ordinary = join(dir, "ordinary.json");
    await writeFile(ordinary, "normal");
    assert.equal(await readFile(ordinary, "utf8"), "normal");
    await promisify(fs.writeFile)(ordinary, "callback");
    assert.equal(await promisify(fs.readFile)(ordinary, "utf8"), "callback");
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
