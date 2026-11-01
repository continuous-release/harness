#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const template = resolve(here, '..', 'sandbox');

const args = process.argv.slice(2);
function option(name, fallback) {
  const index = args.indexOf(name);
  return index === -1 ? fallback : args[index + 1];
}

const repository = option('--repo', process.env.SANDBOX_REPO ?? 'continuous-release/sandbox');
const token = process.env.GITHUB_TOKEN;
if (!token) {
  console.error('GITHUB_TOKEN is required');
  process.exit(1);
}

const remote = `https://x-access-token:${token}@github.com/${repository}.git`;
const dir = mkdtempSync(join(tmpdir(), 'cr-seed-'));

function git(...gitArgs) {
  execFileSync('git', gitArgs, {
    cwd: dir,
    stdio: 'inherit',
    env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
  });
}

try {
  execFileSync('git', ['clone', remote, dir], {
    stdio: 'inherit',
    env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
  });
  git('config', 'user.email', 'harness@continuous-release.invalid');
  git('config', 'user.name', 'continuous-release harness');

  for (const entry of readdirSync(dir)) {
    if (entry === '.git') continue;
    rmSync(join(dir, entry), { recursive: true, force: true });
  }
  cpSync(template, dir, { recursive: true });

  git('add', '-A');
  git('commit', '-m', 'sandbox: base [skip release]');
  git('tag', '-f', 'base');
  git('push', '--force', 'origin', 'main');
  git('push', '--force', 'origin', 'refs/tags/base');

  console.log(`seeded ${repository} from ${template}`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
