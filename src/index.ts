import { parseArgs } from 'node:util';
import { GitHub } from './github.js';
import { runScenario, type ScenarioResult } from './run.js';
import { Sandbox } from './sandbox.js';
import { scenarios } from './scenarios.js';

const USAGE = `continuous-release harness (live GitHub Actions scenarios)

Usage:
  harness list
  harness run [scenario ...] [--all] [--repo owner/repo] [--keep] [--timeout seconds]

Environment:
  GITHUB_TOKEN   token with repo and workflow access
  SANDBOX_REPO   defaults to continuous-release/sandbox
`;

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    repo: { type: 'string' },
    token: { type: 'string' },
    keep: { type: 'boolean', default: false },
    all: { type: 'boolean', default: false },
    timeout: { type: 'string', default: '900' },
    help: { type: 'boolean', default: false },
  },
});

const command = positionals[0] ?? 'list';

if (values.help || command === 'help') {
  process.stdout.write(USAGE);
  process.exit(0);
}

if (command === 'list') {
  for (const scenario of scenarios) {
    console.log(`${scenario.name.padEnd(24)} ${scenario.description}`);
  }
  process.exit(0);
}

if (command !== 'run') {
  console.error(`Unknown command ${command}\n\n${USAGE}`);
  process.exit(1);
}

const repository = values.repo ?? process.env.SANDBOX_REPO ?? 'continuous-release/sandbox';
const token = values.token ?? process.env.GITHUB_TOKEN;
if (!token) {
  console.error('GITHUB_TOKEN is required');
  process.exit(1);
}

const names = positionals.slice(1);
const selected =
  values.all || names.length === 0
    ? scenarios
    : scenarios.filter((scenario) => names.includes(scenario.name));

const missing = names.filter((name) => !scenarios.some((scenario) => scenario.name === name));
if (missing.length > 0) {
  console.error(`Unknown scenarios: ${missing.join(', ')}`);
  process.exit(1);
}

const github = new GitHub({ token, repository });
const sandbox = new Sandbox(repository, token);
const options = {
  keep: values.keep ?? false,
  timeoutMs: Number(values.timeout ?? '900') * 1000,
};

const results: ScenarioResult[] = [];
for (const scenario of selected) {
  try {
    const run = await runScenario(scenario, github, sandbox, options);
    results.push({ name: scenario.name, ok: true, runUrl: run.html_url });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    results.push({ name: scenario.name, ok: false, error: message });
    console.error(`FAILED: ${scenario.name}: ${message}`);
  }
}

console.log('\n=== summary ===');
for (const result of results) {
  const suffix = result.runUrl ? ` ${result.runUrl}` : '';
  const detail = result.error ? `\n  ${result.error}` : '';
  console.log(`${result.ok ? 'PASS' : 'FAIL'} ${result.name}${suffix}${detail}`);
}

const failed = results.filter((result) => !result.ok).length;
console.log(`${results.length - failed}/${results.length} passed`);
process.exit(failed > 0 ? 1 : 0);
