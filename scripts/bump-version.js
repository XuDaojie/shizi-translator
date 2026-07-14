#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const cargoTomlPath = path.join(repoRoot, 'src-tauri', 'Cargo.toml');
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const dryRun = process.argv.includes('--dry-run');

// 仅这些类型参与升版；docs/chore/style/test 等不触发发版
const RELEASE_TYPES = new Set(['feat', 'fix', 'perf']);

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

function parseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`不支持的版本号格式: ${version}`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function formatVersion(version) {
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
  const next = content.replace(/^version = "\d+\.\d+\.\d+"$/m, `version = "${version}"`);
  if (next === content) {
    throw new Error('未找到 Cargo.toml 顶层 version 字段');
  }
  return next;
}

function replaceTauriVersion(content, version) {
  const parsed = JSON.parse(content);
  if (typeof parsed.version !== 'string') {
    throw new Error('tauri.conf.json 缺少 version 字段');
  }
  parsed.version = version;
  return `${JSON.stringify(parsed, null, 2)}\n`;
}

function getLatestSemverTag() {
  const tags = git(['tag', '--list', 'v*', '--sort=-v:refname'])
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^v\d+\.\d+\.\d+$/.test(line));
  return tags[0] || null;
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

  const latestTag = getLatestSemverTag();
  if (!latestTag) {
    throw new Error('未找到形如 vX.Y.Z 的已有 tag，无法推导下一个版本。');
  }

  const range = `${latestTag}..HEAD`;
  const messages = getCommitMessages(range);
  if (messages.length === 0) {
    console.log(`没有发现 ${latestTag} 之后的新提交，不推进版本。`);
    return;
  }

  const currentVersion = parseVersion(latestTag.slice(1));
  const bumpLevel = detectBumpLevel(messages, currentVersion.major);
  if (!bumpLevel) {
    console.log(`没有发现可用于发布的提交（feat/fix/perf/breaking），不推进版本。`);
    return;
  }

  const nextVersion = formatVersion(bumpVersion(currentVersion, bumpLevel));
  const nextTag = `v${nextVersion}`;
  const releaseCommits = listReleaseCommits(messages);

  console.log(`最近版本: ${latestTag}`);
  console.log(`提交数: ${messages.length}（参与升版: ${releaseCommits.length}）`);
  console.log(`版本级别: ${bumpLevel}`);
  console.log(`下一个版本: ${nextTag}`);
  console.log('依据:');
  for (const message of releaseCommits) {
    const mark = isBreaking(message) ? '!' : ' ';
    console.log(`  ${mark} ${subjectOf(message)}`);
  }

  if (dryRun) {
    console.log('（dry-run，未写文件、未打 tag）');
    return;
  }

  const cargoToml = readFile(cargoTomlPath);
  const tauriConfig = readFile(tauriConfigPath);
  writeFile(cargoTomlPath, replaceCargoVersion(cargoToml, nextVersion));
  writeFile(tauriConfigPath, replaceTauriVersion(tauriConfig, nextVersion));

  git(['add', '--', cargoTomlPath, tauriConfigPath]);
  git(['commit', '-m', `chore(release): 发布 ${nextTag}`]);
  git(['tag', '-a', nextTag, '-m', nextTag]);

  console.log(`已创建提交与标签: ${nextTag}`);
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
