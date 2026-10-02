# Release preparation with Sampo

[Sampo](https://github.com/bruits/sampo) turns reviewed changesets into version bumps
and package changelogs. Both Cargo packages form one fixed version group because
they inherit the workspace version. They are marked `publish = false`: distributions
are Windows binaries attached to GitHub Releases, not crates.io packages.

## Adding a changeset

For a user-visible change, add an English Markdown file to `.sampo/changesets`:

```markdown
---
cargo/wh3-mod-manager: patch (Fixed)
---

Keep the selected mod visible after changing its load order.
```

Use `cargo/wh3-core` for core behavior. Include both package keys when the description
applies to both. Bumps are `patch`, `minor` or `major`; section names are `Added`,
`Changed`, `Fixed`, `Removed` and `Security`. Describe the user-visible outcome.
Documentation-only changes do not require a changeset.

For the CLI, install Sampo with `cargo install sampo --locked`, then use `sampo add`
or create the Markdown file directly. `SAMPO_RELEASE_BRANCH=main sampo release --dry-run`
previews the plan on a feature branch. Do not run `sampo publish` for this application.

## Automated flow

1. CI validates the pending release plan on PRs alongside formatting and tests.
2. After a successful push to `main`, the preparation job runs Sampo in `release`
   mode to bump the workspace version, consume changesets and update the two package
   changelogs. It retains existing dependency locks while updating first-party versions.
3. The job creates or refreshes the bot-owned draft PR `release/main`. Review the
   changelog and mark the PR **Ready for review** to run CI on its exact commit.
   Each automatic update returns the PR to draft so it can be checked again.
4. Merge the checked release PR, then publish a GitHub Release with the matching
   `v<workspace version>` tag on that merge commit. Use the desktop changelog entry
   as the release notes; the core changelog provides implementation-level details.
5. `release.yml` builds the published release's tag and uploads its Windows assets.

The repository uses only `ci.yml` and `release.yml`. Sampo prepares files, not tags,
registry uploads or GitHub Releases. The built-in `GITHUB_TOKEN` creates the draft PR;
GitHub suppresses workflows triggered by that token, so CI listens for the human
`ready_for_review` event. No personal token is needed. The repository setting
**Allow GitHub Actions to create and approve pull requests** must be enabled.

Generated changelogs live in `crates/core/CHANGELOG.md` and
`crates/desktop/CHANGELOG.md`. Edit pending changesets on feature branches rather
than editing the bot-owned release branch; it is regenerated from `main`.
