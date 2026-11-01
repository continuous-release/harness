import { GitHub, type WorkflowRun } from './github.js';
import { Sandbox } from './sandbox.js';
import { DEFAULT_CONFIG, type Scenario } from './scenarios.js';

export interface RunOptions {
  keep: boolean;
  timeoutMs: number;
}

export interface ScenarioResult {
  name: string;
  ok: boolean;
  error?: string;
  runUrl?: string;
}

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function matchGlob(name: string, pattern: string): boolean {
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '.*')
    .replace(/\?/g, '.');
  return new RegExp(`^${escaped}$`).test(name);
}

function versionTags(tags: Array<{ ref: string }>): string[] {
  return tags
    .map((tag) => tag.ref.replace('refs/tags/', ''))
    .filter((tag) => tag.startsWith('v'))
    .sort();
}

async function waitForRun(
  github: GitHub,
  workflow: string,
  predicate: (run: WorkflowRun) => boolean,
  timeoutMs: number,
): Promise<WorkflowRun> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const run = (await github.listRuns(workflow)).find(predicate);
    if (run && run.status === 'completed') return run;
    await sleep(5000);
  }
  throw new Error(`Timed out waiting for ${workflow} after ${Math.round(timeoutMs / 1000)}s`);
}

async function dispatchAndWait(
  github: GitHub,
  workflow: string,
  ref: string,
  timeoutMs: number,
): Promise<WorkflowRun> {
  const before = await github.listRuns(workflow);
  const maxId = before.reduce((max, run) => Math.max(max, run.id), 0);
  await github.dispatchWorkflow(workflow, ref);
  return waitForRun(
    github,
    workflow,
    (run) => run.id > maxId && run.event === 'workflow_dispatch',
    timeoutMs,
  );
}

async function assertRelease(
  github: GitHub,
  scenario: Scenario,
  run: WorkflowRun,
): Promise<void> {
  const expected = scenario.expect;

  if (expected.workflow === 'failure') {
    assert(
      run.conclusion === 'failure',
      `expected a failed workflow, got ${run.conclusion}: ${run.html_url}`,
    );
  } else {
    assert(run.conclusion === 'success', `workflow failed: ${run.html_url}`);
  }

  if (expected.release === false) {
    const releases = await github.listReleases();
    assert(
      releases.length === 0,
      `expected no releases, got ${releases.map((release) => release.tag_name).join(', ')}`,
    );
    const tags = versionTags(await github.listTagRefs());
    const initial = [...(scenario.initialTags ?? [])].sort();
    assert(
      JSON.stringify(tags) === JSON.stringify(initial),
      `expected tags ${JSON.stringify(initial)}, got ${JSON.stringify(tags)}`,
    );
    return;
  }

  assert(expected.tag, 'scenario must set expect.tag when a release is expected');
  const release = await github.getReleaseByTag(expected.tag);
  assert(release, `release ${expected.tag} was not created`);

  if (expected.prerelease !== undefined) {
    assert(
      release.prerelease === expected.prerelease,
      `expected prerelease=${expected.prerelease}, got ${release.prerelease}`,
    );
  }

  for (const pattern of expected.assets ?? []) {
    assert(
      release.assets.some((asset) => matchGlob(asset.name, pattern)),
      `no asset matched ${pattern}; assets: ${release.assets.map((asset) => asset.name).join(', ')}`,
    );
  }

  for (const text of expected.notesInclude ?? []) {
    assert(release.body?.includes(text), `notes did not include ${JSON.stringify(text)}`);
  }

  for (const text of expected.notesExclude ?? []) {
    assert(!release.body?.includes(text), `notes unexpectedly included ${JSON.stringify(text)}`);
  }
}

async function runPullRequestScenario(
  scenario: Scenario,
  github: GitHub,
  sandbox: Sandbox,
  options: RunOptions,
): Promise<WorkflowRun> {
  const branch = `scenario/${scenario.name}`;
  await github.deleteBranch(branch);

  let prNumber: number | undefined;
  try {
    sandbox.checkoutNewBranch(branch);
    const config = { ...DEFAULT_CONFIG, ...scenario.config };
    const commits = scenario.commits ?? [];
    for (let i = 0; i < commits.length; i += 1) {
      const commit = commits[i];
      const files =
        i === 0
          ? { ...(commit.files ?? {}), 'scenario.json': `${JSON.stringify(config, null, 2)}\n` }
          : commit.files;
      sandbox.commit(commit.message, commit.body, files);
    }

    const sha = sandbox.head();
    sandbox.pushBranch(branch);

    const pr = await github.createPullRequest({
      title: `harness: ${scenario.name}`,
      head: branch,
      base: 'main',
      body: scenario.description,
    });
    prNumber = pr.number;
    console.log(`opened ${pr.html_url}`);

    const run = await waitForRun(
      github,
      'ci.yml',
      (candidate) => candidate.event === 'pull_request' && candidate.head_branch === branch,
      options.timeoutMs,
    );
    assert(run.conclusion === 'success', `pull request CI failed: ${run.html_url}`);

    const releaseRuns = await github.listRuns('release.yml', sha);
    assert(
      releaseRuns.length === 0,
      `release workflow ran for a pull request: ${releaseRuns.map((candidate) => candidate.html_url).join(', ')}`,
    );

    await assertRelease(github, scenario, run);
    return run;
  } finally {
    if (prNumber !== undefined) {
      await github.closePullRequest(prNumber).catch(() => undefined);
    }
    await github.deleteBranch(branch).catch(() => undefined);
    sandbox.reset();
    sandbox.deleteLocalBranch(branch);
  }
}

export async function runScenario(
  scenario: Scenario,
  github: GitHub,
  sandbox: Sandbox,
  options: RunOptions,
): Promise<WorkflowRun> {
  console.log(`\n=== ${scenario.name} ===`);
  console.log(scenario.description);

  await github.deleteAllReleases();
  await github.deleteAllVersionTags();
  sandbox.reset();

  if (scenario.trigger === 'pull_request') {
    return runPullRequestScenario(scenario, github, sandbox, options);
  }

  const config = { ...DEFAULT_CONFIG, ...scenario.config };
  for (const tag of scenario.initialTags ?? []) {
    sandbox.tag(tag);
    sandbox.pushTag(tag);
  }

  const commits = scenario.commits ?? [];
  for (let i = 0; i < commits.length; i += 1) {
    const commit = commits[i];
    const files =
      i === 0
        ? { ...(commit.files ?? {}), 'scenario.json': `${JSON.stringify(config, null, 2)}\n` }
        : commit.files;
    sandbox.commit(commit.message, commit.body, files);
  }

  if (commits.length === 0 && scenario.config) {
    sandbox.writeScenario(config);
    sandbox.commit('chore: configure the harness scenario');
  }

  const sha = sandbox.head();
  sandbox.pushMain();

  let run: WorkflowRun;
  if (scenario.trigger === 'dispatch') {
    run = await dispatchAndWait(github, 'release.yml', 'main', options.timeoutMs);
  } else {
    run = await waitForRun(
      github,
      'release.yml',
      (candidate) => candidate.event === 'push' && candidate.head_sha === sha,
      options.timeoutMs,
    );
  }

  console.log(`workflow: ${run.html_url} (${run.conclusion})`);
  await assertRelease(github, scenario, run);

  if (scenario.rerun) {
    console.log('dispatching the same commit again');
    const rerun = await dispatchAndWait(github, 'release.yml', 'main', options.timeoutMs);
    console.log(`rerun: ${rerun.html_url} (${rerun.conclusion})`);
    await assertRelease(github, scenario, rerun);

    const tags = versionTags(await github.listTagRefs());
    const expectedCount = (scenario.initialTags?.length ?? 0) + 1;
    assert(
      tags.length === expectedCount,
      `expected ${expectedCount} version tags after a re-run, got ${JSON.stringify(tags)}`,
    );

    assert(scenario.expect.tag, 'scenario must set expect.tag');
    const release = await github.getReleaseByTag(scenario.expect.tag);
    assert(
      release?.assets.length === 1,
      `expected one asset after a re-run, got ${release?.assets.length}`,
    );
  }

  if (!options.keep) {
    await github.deleteAllReleases();
    await github.deleteAllVersionTags();
    sandbox.reset();
    sandbox.pushMain();
  }

  return run;
}
