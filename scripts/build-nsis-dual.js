/**
 * 构建双 NSIS 安装包：
 * - 轻量：`Shizi_*_x64-setup.exe`（downloadBootstrapper，默认 tauri.conf.json）
 * - 完整：`Shizi_*_x64-setup-full.exe`（offlineInstaller，tauri.conf.full.json 合并）
 *
 * 用法（仓库根目录）：node scripts/build-nsis-dual.js
 * 完整包会内嵌 Evergreen 离线安装器（约 +127MB），装到系统共享 Runtime，不是应用私有。
 */
const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const nsisDir = path.join(root, "src-tauri", "target", "release", "bundle", "nsis");
const stagingDir = path.join(nsisDir, ".dual-staging");

function run(cmd, args) {
  console.log(`\n> ${cmd} ${args.join(" ")}\n`);
  const r = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: process.env,
  });
  if (r.status !== 0) {
    process.exit(r.status ?? 1);
  }
}

function listSetupExes(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((n) => n.toLowerCase().endsWith("-setup.exe"))
    .filter((n) => !n.toLowerCase().includes("full"))
    .map((n) => path.join(dir, n));
}

function ensureDir(p) {
  fs.mkdirSync(p, { recursive: true });
}

function copyFile(src, dest) {
  ensureDir(path.dirname(dest));
  fs.copyFileSync(src, dest);
  console.log(`copied: ${path.basename(src)} -> ${dest}`);
}

function main() {
  // 1) 轻量包
  run("npm", ["run", "tauri", "--", "build", "--bundles", "nsis"]);

  const slimFiles = listSetupExes(nsisDir);
  if (slimFiles.length === 0) {
    console.error("轻量 NSIS 安装包未生成");
    process.exit(1);
  }

  ensureDir(stagingDir);
  for (const f of slimFiles) {
    copyFile(f, path.join(stagingDir, path.basename(f)));
  }

  // 2) 完整包（合并 offlineInstaller 配置；会覆盖同名 -setup.exe）
  run("npm", [
    "run",
    "tauri",
    "--",
    "build",
    "--bundles",
    "nsis",
    "--config",
    "src-tauri/tauri.conf.full.json",
  ]);

  const fullBuilt = listSetupExes(nsisDir);
  if (fullBuilt.length === 0) {
    console.error("完整 NSIS 安装包未生成");
    process.exit(1);
  }

  for (const f of fullBuilt) {
    const base = path.basename(f);
    // Shizi_x.y.z_x64-setup.exe -> Shizi_x.y.z_x64-setup-full.exe
    if (!base.toLowerCase().endsWith("-setup.exe")) {
      console.error(`意外文件名: ${base}`);
      process.exit(1);
    }
    const fullName = base.replace(/-setup\.exe$/i, "-setup-full.exe");
    const fullPath = path.join(nsisDir, fullName);
    if (fs.existsSync(fullPath)) fs.unlinkSync(fullPath);
    fs.renameSync(f, fullPath);
    console.log(`renamed full: ${base} -> ${fullName}`);
  }

  // 3) 恢复轻量包到 nsis 目录
  for (const name of fs.readdirSync(stagingDir)) {
    copyFile(path.join(stagingDir, name), path.join(nsisDir, name));
  }

  // 清理 staging
  fs.rmSync(stagingDir, { recursive: true, force: true });

  console.log("\n双包产物：");
  for (const name of fs.readdirSync(nsisDir).sort()) {
    if (!name.toLowerCase().endsWith(".exe")) continue;
    const p = path.join(nsisDir, name);
    const mb = (fs.statSync(p).size / (1024 * 1024)).toFixed(1);
    console.log(`  - ${name}  (${mb} MB)`);
  }
}

main();
