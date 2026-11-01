export interface ReleaseAsset {
  id: number;
  name: string;
  size: number;
  browser_download_url: string;
}

export interface Release {
  id: number;
  tag_name: string;
  body: string | null;
  draft: boolean;
  prerelease: boolean;
  html_url: string;
  assets: ReleaseAsset[];
}

export interface TagRef {
  ref: string;
  object: { sha: string; type: string };
}

export interface WorkflowRun {
  id: number;
  name: string;
  event: string;
  status: string;
  conclusion: string | null;
  head_sha: string;
  head_branch: string | null;
  html_url: string;
  created_at: string;
}

export interface PullRequest {
  number: number;
  html_url: string;
  state: string;
}

export interface GitHubOptions {
  token: string;
  repository: string;
  apiUrl?: string;
}

export class GitHub {
  private readonly apiUrl: string;

  constructor(private readonly options: GitHubOptions) {
    this.apiUrl = (options.apiUrl ?? 'https://api.github.com').replace(/\/$/, '');
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(`${this.apiUrl}${path}`, {
      ...init,
      headers: {
        Accept: 'application/vnd.github+json',
        Authorization: `Bearer ${this.options.token}`,
        'X-GitHub-Api-Version': '2022-11-28',
        'User-Agent': 'continuous-release-harness',
        ...(init.headers as Record<string, string> | undefined),
      },
    });

    if (!response.ok) {
      const text = await response.text();
      const error = new Error(
        `GitHub ${init.method ?? 'GET'} ${path}: ${response.status} ${text}`,
      ) as Error & { status?: number };
      error.status = response.status;
      throw error;
    }

    if (response.status === 204) return undefined as T;
    const text = await response.text();
    return (text ? JSON.parse(text) : undefined) as T;
  }

  private async tryGet<T>(path: string): Promise<T | null> {
    try {
      return await this.request<T>(path);
    } catch (error) {
      if ((error as { status?: number }).status === 404) return null;
      throw error;
    }
  }

  async listReleases(): Promise<Release[]> {
    return this.request<Release[]>(
      `/repos/${this.options.repository}/releases?per_page=100`,
    );
  }

  async getReleaseByTag(tag: string): Promise<Release | null> {
    return this.tryGet<Release>(
      `/repos/${this.options.repository}/releases/tags/${encodeURIComponent(tag)}`,
    );
  }

  async deleteRelease(id: number): Promise<void> {
    await this.request<void>(`/repos/${this.options.repository}/releases/${id}`, {
      method: 'DELETE',
    });
  }

  async listTagRefs(): Promise<TagRef[]> {
    return this.request<TagRef[]>(
      `/repos/${this.options.repository}/git/refs/tags?per_page=100`,
    );
  }

  async deleteTag(tag: string): Promise<void> {
    await this.request<void>(
      `/repos/${this.options.repository}/git/refs/tags/${encodeURIComponent(tag)}`,
      { method: 'DELETE' },
    );
  }

  async deleteAllReleases(): Promise<void> {
    for (const release of await this.listReleases()) {
      await this.deleteRelease(release.id);
    }
  }

  async deleteAllVersionTags(): Promise<void> {
    for (const ref of await this.listTagRefs()) {
      const tag = ref.ref.replace('refs/tags/', '');
      if (tag.startsWith('v')) await this.deleteTag(tag);
    }
  }

  async listRuns(workflow: string, headSha?: string): Promise<WorkflowRun[]> {
    const query = new URLSearchParams({ per_page: '30' });
    if (headSha) query.set('head_sha', headSha);
    const data = await this.request<{ workflow_runs: WorkflowRun[] }>(
      `/repos/${this.options.repository}/actions/workflows/${workflow}/runs?${query}`,
    );
    return data.workflow_runs;
  }

  async dispatchWorkflow(workflow: string, ref: string): Promise<void> {
    await this.request<void>(
      `/repos/${this.options.repository}/actions/workflows/${workflow}/dispatches`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ref }),
      },
    );
  }

  async createPullRequest(input: {
    title: string;
    head: string;
    base: string;
    body?: string;
  }): Promise<PullRequest> {
    return this.request<PullRequest>(`/repos/${this.options.repository}/pulls`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    });
  }

  async closePullRequest(number: number): Promise<void> {
    await this.request<void>(`/repos/${this.options.repository}/pulls/${number}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ state: 'closed' }),
    });
  }

  async deleteBranch(branch: string): Promise<void> {
    try {
      await this.request<void>(
        `/repos/${this.options.repository}/git/refs/heads/${encodeURIComponent(branch)}`,
        { method: 'DELETE' },
      );
    } catch (error) {
      if ((error as { status?: number }).status !== 422) throw error;
    }
  }
}
