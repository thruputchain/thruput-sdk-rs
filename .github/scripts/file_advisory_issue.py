#!/usr/bin/env python3
"""File or update the tracking issue for a failed dependency advisory scan.

Invoked by .github/workflows/audit.yml when the scheduled scan fails. Without
this, a failed scheduled run only emails the workflow file's last committer,
which is nobody's inbox in practice.

Comments on the existing open issue rather than opening a fresh one per
failing day, so a standing advisory produces one issue with a comment trail.

Lives here rather than inline in the workflow so that pyflakes can actually
see it: actionlint only runs pyflakes over steps declaring `shell: python`,
not over Python embedded in a bash heredoc.

Reads its configuration from the environment: GITHUB_REPOSITORY, GH_TOKEN,
ISSUE_TITLE, GITHUB_SERVER_URL, GITHUB_RUN_ID.
"""

import json
import os
import urllib.request

API = "https://api.github.com"
PER_PAGE = 100
PAGE_CAP = 20


def call(repo, token, path, data=None):
    """GET (data=None) or POST a repo-scoped REST endpoint, returning JSON."""
    request = urllib.request.Request(
        "{}/repos/{}/{}".format(API, repo, path),
        data=None if data is None else json.dumps(data).encode(),
        headers={
            "Authorization": "Bearer " + token,
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
        },
        method="GET" if data is None else "POST",
    )
    with urllib.request.urlopen(request) as response:
        return json.load(response)


def find_open_issue(repo, token, title):
    """Return the number of the open issue with this exact title, else None.

    The issues endpoint returns pull requests as well as issues, and those PRs
    count toward the page size — so on a busy repo the tracking issue can sit
    past the first page. Missing it would open a duplicate every failing day
    and fragment the history. Filter PRs out and page until exhausted.
    """
    for page in range(1, PAGE_CAP + 1):
        batch = call(
            repo, token, "issues?state=open&per_page={}&page={}".format(PER_PAGE, page)
        )
        for issue in batch:
            if "pull_request" not in issue and issue["title"] == title:
                return issue["number"]
        if len(batch) < PER_PAGE:
            return None
    print("::warning::gave up after {} pages; may open a duplicate".format(PAGE_CAP))
    return None


def build_body(server_url, repo, run_id):
    run_url = "{}/{}/actions/runs/{}".format(server_url, repo, run_id)
    return (
        "The daily `cargo deny check advisories` run failed: " + run_url + "\n\n"
        "A new advisory may have been filed against a dependency already "
        "pinned in `Cargo.lock`. Reproduce locally with:\n\n"
        "    cargo deny --all-features --locked check advisories\n"
    )


def main():
    repo = os.environ["GITHUB_REPOSITORY"]
    token = os.environ["GH_TOKEN"]
    title = os.environ["ISSUE_TITLE"]
    body = build_body(
        os.environ["GITHUB_SERVER_URL"], repo, os.environ["GITHUB_RUN_ID"]
    )

    existing = find_open_issue(repo, token, title)
    if existing is not None:
        print("Commenting on existing issue #{}".format(existing))
        call(repo, token, "issues/{}/comments".format(existing), {"body": body})
    else:
        print("Opening new issue")
        call(repo, token, "issues", {"title": title, "body": body})


if __name__ == "__main__":
    main()
