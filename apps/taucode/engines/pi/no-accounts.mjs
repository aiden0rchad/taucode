// Pi 1.4.0's FileAuthStorageBackend creates {} on every first credential read.
// Keep that store empty without creating auth.json. Registry models resolve their
// keys independently from env/commands; any attempt to persist credentials fails.
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { resolve, basename, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const agentDir = process.env.PI_CODING_AGENT_DIR;
if (!agentDir) throw new Error("Taucode requires an isolated PI_CODING_AGENT_DIR");
const authPaths = ["auth.json", "mcp-auth.json"].map(name => resolve(agentDir, name));
const isAuth = (path) => {
  if (path instanceof URL) path = fileURLToPath(path);
  if (Buffer.isBuffer(path)) path = path.toString();
  return typeof path === "string" && authPaths.includes(resolve(path));
};
const read = fs.readFileSync;
const write = fs.writeFileSync;
fs.readFileSync = function (path, options) {
  const resolved = typeof path === "string" ? resolve(path) : null;
  if (resolved && basename(resolved) === "mcp.json" && basename(dirname(resolved)) === ".pi" && dirname(resolved) !== resolve(agentDir)) {
    return typeof options === "string" || options?.encoding ? "{}" : Buffer.from("{}");
  }
  if (!isAuth(path)) return read.call(this, path, options);
  return typeof options === "string" || options?.encoding ? "{}" : Buffer.from("{}");
};
fs.writeFileSync = function (path, data, options) {
  if (!isAuth(path)) return write.call(this, path, data, options);
  if (String(data).trim() !== "{}") {
    throw new Error("Taucode disables accounts and credential files. Configure an API key provider in Settings.");
  }
};
const empty = options => typeof options === "string" || options?.encoding ? "{}" : Buffer.from("{}");
const denied = data => String(data).trim() !== "{}"
  ? new Error("Taucode disables accounts and credential files. Configure an API key provider in Settings.") : null;
const readPromise = fs.promises.readFile;
const writePromise = fs.promises.writeFile;
fs.promises.readFile = async function (path, options) {
  return isAuth(path) ? empty(options) : readPromise.call(this, path, options);
};
fs.promises.writeFile = async function (path, data, options) {
  if (!isAuth(path)) return writePromise.call(this, path, data, options);
  const error = denied(data);
  if (error) throw error;
};
const readCallback = fs.readFile;
const writeCallback = fs.writeFile;
fs.readFile = function (path, options, callback) {
  if (!isAuth(path)) return readCallback.apply(this, arguments);
  if (typeof options === "function") { callback = options; options = undefined; }
  queueMicrotask(() => callback(null, empty(options)));
};
fs.writeFile = function (path, data, options, callback) {
  if (!isAuth(path)) return writeCallback.apply(this, arguments);
  if (typeof options === "function") callback = options;
  queueMicrotask(() => callback(denied(data)));
};
syncBuiltinESMExports();
