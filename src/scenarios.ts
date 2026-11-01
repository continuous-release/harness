export interface SandboxConfig {
  system: string;
  directory: string;
  setup: string;
  test: string;
  build: string;
  publish: string;
  artifact: string;
}

export const DEFAULT_CONFIG: SandboxConfig = {
  system: 'node',
  directory: 'fixtures/node',
  setup: 'npm install --ignore-scripts',
  test: 'npm test',
  build:
    'mkdir -p dist && npm version "$VERSION" --no-git-tag-version --allow-same-version && npm pack --pack-destination dist',
  publish: '',
  artifact: 'dist/*.tgz',
};

export interface CommitSpec {
  message: string;
  body?: string;
  files?: Record<string, string>;
}

export interface Scenario {
  name: string;
  description: string;
  initialTags?: string[];
  commits?: CommitSpec[];
  config?: Partial<SandboxConfig>;
  trigger?: 'push' | 'dispatch' | 'pull_request';
  rerun?: boolean;
  expect: {
    workflow?: 'success' | 'failure';
    release?: boolean;
    tag?: string;
    prerelease?: boolean;
    assets?: string[];
    notesInclude?: string[];
    notesExclude?: string[];
  };
}

export const scenarios: Scenario[] = [
  {
    name: 'first-release',
    description: 'With no tags, the first feature starts at 0.1.0.',
    commits: [{ message: 'feat: add the first feature' }],
    expect: {
      tag: 'v0.1.0',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### Features', 'add the first feature'],
    },
  },
  {
    name: 'patch',
    description: 'A fix after v1.2.3 bumps the patch version.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'fix: correct the timeout' }],
    expect: {
      tag: 'v1.2.4',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### Bug Fixes', 'correct the timeout'],
    },
  },
  {
    name: 'minor',
    description: 'A feature after v1.2.3 bumps the minor version.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'feat: add a feature' }],
    expect: {
      tag: 'v1.3.0',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### Features', 'add a feature'],
    },
  },
  {
    name: 'major-bang',
    description: 'A bang on the commit type bumps the major version.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'feat!: drop the old API' }],
    expect: {
      tag: 'v2.0.0',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### ⚠ BREAKING CHANGES', 'drop the old API'],
    },
  },
  {
    name: 'major-footer',
    description: 'A BREAKING CHANGE footer bumps the major version.',
    initialTags: ['v1.2.3'],
    commits: [
      {
        message: 'fix: adjust the API',
        body: 'BREAKING CHANGE: removed the old flag',
      },
    ],
    expect: {
      tag: 'v2.0.0',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### ⚠ BREAKING CHANGES', 'adjust the API'],
    },
  },
  {
    name: 'chore-patch',
    description: 'Every releasable commit releases; a chore is a patch.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'chore: tidy the repository' }],
    expect: {
      tag: 'v1.2.4',
      release: true,
      assets: ['*.tgz'],
      notesInclude: ['### Chores', 'tidy the repository'],
    },
  },
  {
    name: 'release-as',
    description: 'Release-As forces an exact version.',
    initialTags: ['v1.2.3'],
    commits: [
      {
        message: 'chore: force the release',
        body: 'Release-As: 3.4.5',
      },
    ],
    expect: {
      tag: 'v3.4.5',
      release: true,
      assets: ['*.tgz'],
    },
  },
  {
    name: 'prerelease',
    description: 'A prerelease version marks the release as a prerelease.',
    initialTags: ['v1.2.3'],
    commits: [
      {
        message: 'chore: release candidate',
        body: 'Release-As: 1.3.0-rc.1',
      },
    ],
    expect: {
      tag: 'v1.3.0-rc.1',
      release: true,
      prerelease: true,
      assets: ['*.tgz'],
    },
  },
  {
    name: 'notes-grouping',
    description: 'Notes group commits and link the compare range.',
    initialTags: ['v1.2.3'],
    commits: [
      { message: 'feat: add a feature' },
      { message: 'fix: correct a bug' },
      { message: 'docs: explain the feature' },
    ],
    expect: {
      tag: 'v1.3.0',
      release: true,
      assets: ['*.tgz'],
      notesInclude: [
        '### Features',
        '### Bug Fixes',
        '### Documentation',
        'compare/v1.2.3...v1.3.0',
      ],
    },
  },
  {
    name: 'idempotent-rerun',
    description: 'Re-running the same commit updates the release and keeps the tag.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'fix: correct the timeout' }],
    rerun: true,
    expect: {
      tag: 'v1.2.4',
      release: true,
      assets: ['*.tgz'],
    },
  },
  {
    name: 'existing-tag-at-head',
    description: 'A tag already on HEAD updates that release instead of bumping.',
    initialTags: ['v1.2.3'],
    trigger: 'dispatch',
    expect: {
      tag: 'v1.2.3',
      release: true,
      assets: ['*.tgz'],
    },
  },
  {
    name: 'build-failure',
    description: 'A failed build leaves no release and no new tag.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'fix: correct the timeout' }],
    config: { build: 'exit 1' },
    expect: { workflow: 'failure', release: false },
  },
  {
    name: 'publish-failure',
    description: 'A failed publish leaves no release and no new tag.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'feat: add a feature' }],
    config: { publish: 'exit 1' },
    expect: { workflow: 'failure', release: false },
  },
  {
    name: 'missing-artifact',
    description: 'A missing artifact fails before the release is created.',
    initialTags: ['v1.2.3'],
    commits: [{ message: 'fix: correct the timeout' }],
    config: { artifact: 'dist/nope-*.tgz' },
    expect: { workflow: 'failure', release: false },
  },
  {
    name: 'pr-no-release',
    description: 'A pull request builds and tests, but never releases.',
    trigger: 'pull_request',
    commits: [{ message: 'feat: propose a feature' }],
    expect: { workflow: 'success', release: false },
  },
];
