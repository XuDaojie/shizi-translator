#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const cargoTomlPath = path.join(repoRoot, 'src-tauri', 'Cargo.toml');
const cargoLockPath = path.join(repoRoot, 'src-tauri', 'Cargo.lock');
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
// 预览：--dry 或 --dry-run（等价）；落盘加 --beta 走预发布
const dryRun = process.argv.includes('--dry-run') || process.argv.includes('--dry');
const beta = process.argv.includes('--beta');

// 仅这些类型参与升版；docs/chore/style/test 等不触发发版
const RELEASE_TYPES = new Set(['feat', 'fix', 'perf']);
// 版本写入：X.Y.Z 或 X.Y.Z-beta.N
const VERSION_RE = /^\d+\.\d+\.\d+(?:-beta\.\d+)?$/;

function git(args) {
  return execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
  }).trim();
}

function readFile(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function writeFile(filePath, content) {
  fs.writeFileSync(filePath, content, 'utf8');
}

function parseStableVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`不支持的正式版本号格式: ${version}`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function formatStableVersion(version) {
  return `${version.major}.${version.minor}.${version.patch}`;
}

function bumpVersion(version, level) {
  if (level === 'major') {
    return { major: version.major + 1, minor: 0, patch: 0 };
  }
  if (level === 'minor') {
    return { major: version.major, minor: version.minor + 1, patch: 0 };
  }
  return { major: version.major, minor: version.minor, patch: version.patch + 1 };
}

function replaceCargoVersion(content, version) {
  if (!VERSION_RE.test(version)) {
    throw new Error(`拒绝写入非法版本号: ${version}`);
  }
  const next = content.replace(
    /^version = "\d+\.\d+\.\d+(?:-beta\.\d+)?"$/m,
    `version = "${version}"`,
  );
  if (next === content) {
    throw new Error('未找到 Cargo.toml 顶层 version 字段');
  }
  return next;
}

// 本地包条目：[[package]]\nname = "shizi"\nversion = "..."
// cargo build 也会改这一行；打 tag 时一并写入，避免发布后工作区脏
function replaceCargoLockVersion(content, version) {
  if (!VERSION_RE.test(version)) {
    throw new Error(`拒绝写入非法版本号: ${version}`);
  }
  const next = content.replace(
    /(\[\[package\]\]\r?\nname = "shizi"\r?\nversion = ")[^"]+(")/,
    `$1${version}$2`,
  );
  if (next === content) {
    throw new Error('未找到 Cargo.lock 中 shizi 包的 version 字段');
  }
  return next;
}

function replaceTauriVersion(content, version) {
  if (!VERSION_RE.test(version)) {
    throw new Error(`拒绝写入非法版本号: ${version}`);
  }
  const parsed = JSON.parse(content);
  if (typeof parsed.version !== 'string') {
    throw new Error('tauri.conf.json 缺少 version 字段');
  }
  parsed.version = version;
  return `${JSON.stringify(parsed, null, 2)}\n`;
}

function listTags() {
  return git(['tag', '--list', 'v*', '--sort=-v:refname'])
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function getLatestStableTag() {
  const tags = listTags().filter((line) => /^v\d+\.\d+\.\d+$/.test(line));
  return tags[0] || null;
}

/** 目标正式号 T 下已有的 beta tag，按序号升序 */
function listBetaTagsFor(targetStable) {
  const re = new RegExp(`^v${targetStable.replace(/\./g, '\\.')}-beta\\.(\\d+)$`);
  return listTags()
    .map((tag) => {
      const match = re.exec(tag);
      return match ? { tag, n: Number(match[1]) } : null;
    })
    .filter(Boolean)
    .sort((a, b) => a.n - b.n);
}

function getCommitMessages(range) {
  const output = git(['log', '--format=%s%n%b%x00', range]);
  return output
    .split('\0')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function subjectOf(message) {
  return message.split(/\r?\n/, 1)[0];
}

function isBreaking(message) {
  const firstLine = subjectOf(message);
  return /BREAKING CHANGE:/i.test(message) || /^[a-z]+(\([^)]+\))?!:/.test(firstLine);
}

function conventionalType(message) {
  const match = /^([a-z]+)(\([^)]+\))?!?:/.exec(subjectOf(message));
  return match ? match[1] : null;
}

/**
 * 0.x：breaking → minor；1.0+：breaking → major
 * 仅 feat / fix / perf / breaking 参与升版
 */
function detectBumpLevel(messages, currentMajor) {
  let level = null;
  for (const message of messages) {
    if (isBreaking(message)) {
      // ponytail: 0.x 不进 1.0，与项目 SemVer 约定一致
      return currentMajor === 0 ? 'minor' : 'major';
    }
    const type = conventionalType(message);
    if (!type || !RELEASE_TYPES.has(type)) {
      continue;
    }
    if (type === 'feat') {
      level = 'minor';
      continue;
    }
    // fix / perf
    if (!level) {
      level = 'patch';
    }
  }
  return level;
}

function listReleaseCommits(messages) {
  return messages.filter((message) => {
    if (isBreaking(message)) {
      return true;
    }
    const type = conventionalType(message);
    return type && RELEASE_TYPES.has(type);
  });
}

function ensureCleanWorktree() {
  const status = git(['status', '--porcelain']);
  if (status) {
    throw new Error('工作区不干净，拒绝自动发布。请先提交或清理现有改动。');
  }
}

function main() {
  if (!dryRun) {
    ensureCleanWorktree();
  }

  const latestStableTag = getLatestStableTag();
  if (!latestStableTag) {
    throw new Error('未找到形如 vX.Y.Z 的已有正式 tag，无法推导下一个版本。');
  }

  const range = `${latestStableTag}..HEAD`;
  const messages = getCommitMessages(range);
  if (messages.length === 0) {
    console.log(`没有发现 ${latestStableTag} 之后的新提交，不推进版本。`);
    return;
  }

  const currentVersion = parseStableVersion(latestStableTag.slice(1));
  const bumpLevel = detectBumpLevel(messages, currentVersion.major);
  if (!bumpLevel) {
    console.log(`没有发现可用于发布的提交（feat/fix/perf/breaking），不推进版本。`);
    return;
  }

  const targetStable = formatStableVersion(bumpVersion(currentVersion, bumpLevel));
  const releaseCommits = listReleaseCommits(messages);

  let nextVersion;
  let existingBetas = [];
  if (beta) {
    existingBetas = listBetaTagsFor(targetStable);
    const nextN = existingBetas.length ? existingBetas[existingBetas.length - 1].n + 1 : 1;
    nextVersion = `${targetStable}-beta.${nextN}`;
  } else {
    nextVersion = targetStable;
  }
  const nextTag = `v${nextVersion}`;

  console.log(`通道: ${beta ? 'beta（预发布）' : 'release（正式）'}`);
  console.log(`最近正式版: ${latestStableTag}`);
  console.log(`提交数: ${messages.length}（参与升版: ${releaseCommits.length}）`);
  console.log(`版本级别: ${bumpLevel}`);
  console.log(`目标正式号: ${targetStable}`);
  if (beta) {
    const listed = existingBetas.length
      ? existingBetas.map((item) => item.tag).join(', ')
      : '（无）';
    console.log(`已有预发布: ${listed}`);
  }
  console.log(`下一个版本: ${nextTag}`);
  console.log('依据:');
  for (const message of releaseCommits) {
    const mark = isBreaking(message) ? '!' : ' ';
    console.log(`  ${mark} ${subjectOf(message)}`);
  }

  if (dryRun) {
    console.log('（dry，未写文件、未打 tag）');
    return;
  }

  const cargoToml = readFile(cargoTomlPath);
  const cargoLock = readFile(cargoLockPath);
  const tauriConfig = readFile(tauriConfigPath);
  writeFile(cargoTomlPath, replaceCargoVersion(cargoToml, nextVersion));
  writeFile(cargoLockPath, replaceCargoLockVersion(cargoLock, nextVersion));
  writeFile(tauriConfigPath, replaceTauriVersion(tauriConfig, nextVersion));

  git(['add', '--', cargoTomlPath, cargoLockPath, tauriConfigPath]);
  git(['commit', '-m', `chore(release): 发布 ${nextTag}`]);
  git(['tag', '-a', nextTag, '-m', nextTag]);

  console.log(`已创建提交与标签: ${nextTag}`);
}

if (process.argv.includes('--self-check')) {
  // ponytail: 最小自检，不拉起 git
  const sample = '[[package]]\nname = "shizi"\nversion = "0.6.1"\ndependencies = [\n';
  const got = replaceCargoLockVersion(sample, '0.7.0-beta.1');
  if (!got.includes('version = "0.7.0-beta.1"')) {
    console.error('self-check failed: lock version not replaced');
    process.exit(1);
  }
  console.log('self-check ok');
  process.exit(0);
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
