// Pi 1.4.0's FileAuthStorageBackend creates {} on every first credential read.
// Keep that store empty without creating auth.json. Registry models resolve their
// keys independently from env/commands; any attempt to persist credentials fails.
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const agentDir = process.env.PI_CODING_AGENT_DIR;
if (!agentDir) throw new Error("Taucode requires an isolated PI_CODING_AGENT_DIR");
const authPath = resolve(agentDir, "auth.json");
const isAuth = (path) => {
  if (path instanceof URL) path = fileURLToPath(path);
  if (Buffer.isBuffer(path)) path = path.toString();
  return typeof path === "string" && resolve(path) === authPath;
};
const read = fs.readFileSync;
const write = fs.writeFileSync;
fs.readFileSync = function (path, options) {
  if (!isAuth(path)) return read.call(this, path, options);
  return typeof options === "string" || options?.encoding ? "{}" : Buffer.from("{}");
};
fs.writeFileSync = function (path, data, options) {
  if (!isAuth(path)) return write.call(this, path, data, options);
  if (String(data).trim() !== "{}") {
    throw new Error("Taucode disables accounts and credential files. Configure an API key provider in Settings.");
  }
};
syncBuiltinESMExports();
