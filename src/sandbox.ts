import { execFileSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import type { SandboxConfig } from './scenarios.js';

export class Sandbox {
  readonly dir: string;
  private readonly remote: string;

  constructor(
    repository: string,
    token: string,
    workDir?: string,
  ) {
    this.dir = workDir ?? mkdtempSync(join(tmpdir(), 'cr-sandbox-'));
    this.remote = `https://x-access-token:${token}@github.com/${repository}.git`;
  }

  private git(...args: string[]): string {
    try {
      return execFileSync('git', args, {
        cwd: this.dir,
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
      }).trim();
    } catch (error) {
      const failure = error as { message: string; stderr?: string | Buffer };
      const detail = failure.stderr ? `\n${failure.stderr.toString().trim()}` : '';
      throw new Error(`${failure.message}${detail}`);
    }
  }

  private ensureClone(): void {
    if (existsSync(join(this.dir, '.git'))) return;
    execFileSync('git', ['clone', this.remote, this.dir], {
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
    });
    this.git('config', 'user.email', 'harness@continuous-release.invalid');
    this.git('config', 'user.name', 'continuous-release harness');
  }

  reset(): void {
    this.ensureClone();
    this.git('fetch', 'origin', 'refs/tags/base:refs/tags/base', '--force');
    this.git('checkout', '-B', 'main', 'base');
    this.git('reset', '--hard', 'base');
    this.git('clean', '-fdx');
    for (const tag of this.git('tag', '-l', 'v*').split('\n').filter(Boolean)) {
      this.git('tag', '-d', tag);
    }
  }

  writeScenario(config: SandboxConfig): void {
    writeFileSync(join(this.dir, 'scenario.json'), `${JSON.stringify(config, null, 2)}\n`);
  }

  commit(message: string, body?: string, files: Record<string, string> = {}): string {
    for (const [path, content] of Object.entries(files)) {
      const target = join(this.dir, path);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content);
    }

    if (!files['CHANGELOG.md']) {
      appendFileSync(join(this.dir, 'CHANGELOG.md'), `\n- ${message}\n`);
    }

    this.git('add', '-A');
    const args = ['commit', '-m', message];
    if (body) args.push('-m', body);
    this.git(...args);
    return this.head();
  }

  tag(name: string): void {
    this.git('tag', '-f', name);
  }

  pushTag(name: string): void {
    this.git('push', '--force', 'origin', `refs/tags/${name}`);
  }

  pushMain(): void {
    this.git('push', '--force', 'origin', 'main');
  }

  pushBranch(name: string): void {
    this.git('push', '--force', 'origin', `${name}:${name}`);
  }

  checkoutNewBranch(name: string): void {
    this.git('checkout', '-B', name, 'base');
  }

  deleteLocalBranch(name: string): void {
    try {
      this.git('branch', '-D', name);
    } catch {
      // The branch may already be gone.
    }
  }

  head(): string {
    return this.git('rev-parse', 'HEAD');
  }
}
