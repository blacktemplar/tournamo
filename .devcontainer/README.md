# The dev container

A container for working on Tournamo, made for letting Claude Code work without asking
(`bypassPermissions`), in Zed or in a terminal. It has everything the hooks need, including the
manual ones (`prek run --all-files --stage manual`): Rust stable with the wasm target, nightly
rustfmt, wasm-pack, trunk, dx, Chromium and chromedriver, Node. Also cargo-warloc, to count lines
of code, and gh, reading GitHub with a read-only token (*GitHub CLI* below).

## Using it in Zed

1. Open the checkout (or a worktree of it) in Zed and choose *Open in Container* (or *Project: Open
   Remote* → *Connect Dev Container*). The first build takes a while.
2. Add *Claude Agent* once: `agent: open settings` → *External Agents* → *Add Agent* → *Install
   from Registry*. In a container project Zed runs it inside the container.
3. Sign in once: start a Claude Agent thread and run `/login`, or run `claude` in Zed's terminal. The
   sign-in is kept in the volume `tournamo-claude`, which every checkout shares.

Claude starts in `bypassPermissions` there, set by `/etc/claude-code/managed-settings.json` of the
image, so it applies only inside the container. In a terminal, `claude` does the same.

Zed does not notice changes to these files: after one, remove the containers
(`docker compose -p tournamo-<hash> down`, the name is in `.env`) and open the project again.

A new container has a new id, and Zed files its agent threads and project groups under the
container's id: after it creates one, the threads of the old container neither show in the sidebar
nor open, and the old group stays in the window without opening
([zed#56576](https://github.com/zed-industries/zed/issues/56576)). To move the threads to the new
container and remove the old group, quit Zed, run `.devcontainer/rebind-zed-threads.sh` on the host
(from a terminal outside Zed), and start Zed again.

## GitHub CLI

`gh` reads issues, pull requests and CI runs with a token from the host, which must not be able to
write: the container must stay unable to change anything on GitHub. Make a *fine-grained* personal
access token (Settings → Developer settings → Personal access tokens → Fine-grained tokens):

- *Repository access*: only `blacktemplar/tournamo`.
- *Permissions*, all *Read-only*: Actions, Contents, Issues, Pull requests (Metadata comes with
  them). No account permissions.

Put it into `~/.config/tournamo/gh-token` on the host (`initialize.sh` creates the file empty, mode
600), then remove the containers and open the project again. `gh` refuses classic tokens
(`ghp_...`), whose `repo` scope cannot be read-only; whether a fine-grained one only reads, it
cannot tell, so give it no write permission. Without a token, `gh` says so and stops.

## What it protects

Claude can do anything the user `dev` can do in the container. What it cannot do:

- **Reach the network freely.** The container sits on an internal Docker network without a route
  out; the only way out is the proxy (squid), which tunnels HTTPS to the hosts in
  `proxy/allowed-domains.txt` and refuses everything else. Neither DNS nor any other protocol gets
  out, and since it is enforced outside the container, even root inside could not bypass it. Every
  request is logged: `docker logs tournamo-<hash>-proxy-1`. Claude's WebSearch runs on Anthropic's
  side and still works; WebFetch reaches the allowed hosts only.
- **Become root or change the tools.** No sudo, no capabilities, `no-new-privileges`; the
  toolchains and Claude Code itself are owned by root.
- **Touch the host beyond the checkout.** Mounted are the checkout, the git repository it belongs
  to (for a worktree, the main repository's `.git`), and volumes. No home directory, no SSH keys, no
  git credentials (so no `git push`: push from the host), no Docker socket. `git fetch` does work:
  the image's git config turns GitHub's SSH URLs into HTTPS ones, the only way through the proxy.
- **Plant code the host runs by itself.** Read-only: `.git/hooks`, `.git/config` (it can name
  commands), and `.devcontainer` (whose `initializeCommand` and mounts the host would use).
  `target/` is a volume of the container, so the host never runs what was built inside.

## What remains

- **Everything in the checkout is Claude's output.** Code you build or run on the host (`build.rs`,
  proc macros, tests, `.pre-commit-config.yaml`, which your hook runs on every commit) can do
  anything you can. Read the diff before running anything on the host, or run it in the container
  only. The same holds for commits Claude makes: it could commit a change to `.devcontainer` even
  though it cannot write the files, which a later checkout on the host would bring in; look at
  `git diff` of `.devcontainer` before reopening the container.
- **The repository is writable.** Claude can rewrite branches and other worktrees' refs in the
  shared `.git` (the reflog keeps the old commits) and delete files of the checkout. Keep pushed
  backups.
- **The allowed hosts are channels.** Data can leave to them: to the Anthropic API (inherent), or
  to GitHub only with credentials that can write, which the container has none of (`gh`'s token
  only reads, if you made it so). Its own credentials are the sign-in of Claude Code and that
  token, readable in the container; the token reads no more than the repository, which is public. Anything Claude reads from the allowed hosts
  (a repository, a crate, docs) may try to instruct it (prompt injection); with no secrets and a
  closed network, the damage is limited to the checkout.
- **claude.ai connectors** (Gmail, Drive, ...) would be a way out the proxy cannot see, since they
  run through Anthropic; the managed settings turn them off, and `mcp-proxy.anthropic.com` is not
  allowed either.
- **Containers are not VMs.** The kernel is shared; a kernel exploit escapes. For a stronger
  boundary run Docker in a VM, or use gVisor (`runtime: runsc` in `compose.yaml`).
