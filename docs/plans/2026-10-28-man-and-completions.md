# Plan: install manual pages and shell completions

**File:** `docs/plans/man-and-completions.md`  
**Status:** Proposed  
**Scope:** `terminal-ai` installation and shared generators in `development-tools`.

## Goal

Install reviewed manual pages and shell completions alongside `terminal-ai` when their source files exist.

Keep installation local to the current user by default, while providing configurable destinations and `DESTDIR` staging for a future pacman package.

Generation remains a separate, explicit development operation. Installation must not invoke `terminal-ai`, contact a backend, load project `.env` files, or require the generation tools.

## Repository responsibilities

### `terminal-ai`

Owns and distributes the application resources:

```text
install.sh
bin/terminal-ai
utils/tai-today
man/terminal-ai.1
completions/terminal-ai.fish
completions/terminal-ai.zsh       # Optional.
completions/terminal-ai.bash      # Optional.
```

The existing `bin/terminal-ai` installation source remains the default. Do not switch to a Cargo `target/release` layout implicitly; allow the binary source to be overridden.

Generated candidates remain separate:

```text
man/terminal-ai.1.new
completions/terminal-ai.fish.new
completions/terminal-ai.zsh.new
completions/terminal-ai.bash.new
```

Only reviewed files without `.new` are installable.

### `development-tools`

Owns the shared generation commands and installs them into its existing directory on `PATH`.

Use these command names:

| Source in `development-tools` | Installed command |
| --- | --- |
| `tools/create-completions.sh` | `create-completions` |
| `tools/create-manual.sh` | `create-man` |

Keeping the current source filenames avoids an unnecessary rename during migration. The public manual-generation command will be `create-man`.

These commands operate on the caller’s project, not the `development-tools` checkout.

## Installation contract

### Required and optional resources

The following remain required:

- `bin/terminal-ai`, or the explicitly selected binary source.
- `utils/tai-today`.

The following are independently optional:

- `man/terminal-ai.1`.
- Each supported completion file.

A missing optional resource does not fail installation. A present resource that cannot be read or installed does fail installation.

Missing source resources must not remove previously installed copies. Installation is not a synchronization or cleanup operation.

Use explicit source filenames rather than directory-wide globs. In particular, never install `.new` files or anything from `.generated/`.

### Destinations

Define these configurable installer variables:

| Variable | Default |
| --- | --- |
| `BASE_DIR` | Directory containing the repository-root `install.sh` |
| `APP_BUILD_BIN` | `$BASE_DIR/bin/terminal-ai` |
| `PREFIX` | `$HOME/.local` |
| `BINDIR` | `$PREFIX/bin` |
| `DATADIR` | See prefix rules below |
| `MANDIR` | `$DATADIR/man` |
| `FISH_COMPLETION_DIR` | See prefix rules below |
| `ZSH_COMPLETION_DIR` | `$DATADIR/zsh/site-functions` |
| `BASH_COMPLETION_DIR` | `$DATADIR/bash-completion/completions` |
| `DESTDIR` | Empty |

Prefix rules:

- For the default user prefix, use `${XDG_DATA_HOME:-$HOME/.local/share}` as `DATADIR` and `${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions` as `FISH_COMPLETION_DIR`.
- For another prefix, default `DATADIR` to `$PREFIX/share` and `FISH_COMPLETION_DIR` to `$DATADIR/fish/vendor_completions.d`.
- Explicit destination overrides always take precedence.
- `DESTDIR` is prepended to every installation destination. It must not affect source paths.

Install these exact filenames:

| Source | Destination before `DESTDIR` |
| --- | --- |
| Selected application binary | `$BINDIR/terminal-ai` |
| `utils/tai-today` | `$BINDIR/tai-today` |
| `man/terminal-ai.1` | `$MANDIR/man1/terminal-ai.1` |
| `completions/terminal-ai.fish` | `$FISH_COMPLETION_DIR/terminal-ai.fish` |
| `completions/terminal-ai.zsh` | `$ZSH_COMPLETION_DIR/_terminal-ai` |
| `completions/terminal-ai.bash` | `$BASH_COMPLETION_DIR/terminal-ai` |

Use mode `0755` for commands and `0644` for manual pages and completions.

Do not condition resource installation on whether a shell is installed or its configuration directory already exists. Create destination directories as needed.

## Changes to `terminal-ai/install.sh`

Replace the current working-directory-dependent installer with a Bash script using `set -euo pipefail`.

Implementation steps:

1. Resolve the source root from the script location, with `BASE_DIR` as an override.
2. Resolve installation destinations using the rules above.
3. Validate both required command sources before copying anything.
4. Install the two commands using `install -Dm0755`.
5. Install each existing optional resource using `install -Dm0644`.
6. Print `installed` only after all requested copies succeed.

A suitable optional-resource helper is:

```bash
install_optional() {
    local source="$1"
    local destination="$2"

    if [[ ! -e "$source" && ! -L "$source" ]]; then
        printf 'Skipping missing optional resource: %s\n' "$source" >&2
        return 0
    fi

    if [[ ! -f "$source" || ! -r "$source" ]]; then
        printf 'Cannot read resource: %s\n' "$source" >&2
        return 1
    fi

    install -Dm0644 -- "$source" "${DESTDIR}${destination}"
}
```

Call it explicitly for the four optional resources:

```bash
install_optional \
    "$BASE_DIR/man/terminal-ai.1" \
    "$MANDIR/man1/terminal-ai.1"

install_optional \
    "$BASE_DIR/completions/terminal-ai.fish" \
    "$FISH_COMPLETION_DIR/terminal-ai.fish"

install_optional \
    "$BASE_DIR/completions/terminal-ai.zsh" \
    "$ZSH_COMPLETION_DIR/_terminal-ai"

install_optional \
    "$BASE_DIR/completions/terminal-ai.bash" \
    "$BASH_COMPLETION_DIR/terminal-ai"
```

Retain the simple success-output convention for now. The more elaborate example installer’s status aggregation, development symlinks, and separate `install-user-with-man` command are unnecessary for this change.

The ordinary install operation should install all available resources automatically.

Installation may partially complete if a later filesystem operation fails; this change does not promise transactional installation.

## Changes to the `development-tools` Makefile

Preserve the existing command installation and fish `PATH` setup.

During migration, install the generators if their source files exist:

```makefile
	@if [ -f "$(PATH_SOURCE)/tools/create-completions.sh" ]; then \
		install -Dm0755 "$(PATH_SOURCE)/tools/create-completions.sh" \
			"$(PATH_DESTINATION)/create-completions"; \
	fi
	@if [ -f "$(PATH_SOURCE)/tools/create-manual.sh" ]; then \
		install -Dm0755 "$(PATH_SOURCE)/tools/create-manual.sh" \
			"$(PATH_DESTINATION)/create-man"; \
	fi
```

Add both installed command names to the existing `uninstall` recipe:

```makefile
		"$(PATH_DESTINATION)/create-completions" \
		"$(PATH_DESTINATION)/create-man" \
```

Make the existing `stash.1` installation conditional as well:

```makefile
	@if [ -f "$(MAN_SOURCE)/stash.1" ]; then \
		install -Dm0644 "$(MAN_SOURCE)/stash.1" \
			"$(MAN_DESTINATION)/stash.1"; \
	fi
```

If manuals for the generators are added later, install them explicitly as `create-completions.1` and `create-man.1`, using the same optional-resource policy.

Do not install `terminal-ai.1` or application completions from `development-tools`. Those belong to the application repository.

## Generator migration and corrections

Moving the scripts onto `PATH` must not change which project supplies configuration and inputs.

### Project-local execution

Both generators should:

1. Respect an explicitly supplied `REPO_ROOT`.
2. Otherwise discover the target repository from the caller’s working directory.
3. Default `BASE_PATH` to that repository root.
4. Resolve `BASE_PATH` to an absolute directory.
5. Load only the trusted project-local `$BASE_PATH/.env`.
6. Run with `$BASE_PATH` as their working directory.
7. Respect `TAI_BIN`, defaulting to `terminal-ai`, as the commit-message generator does.

Quote paths consistently, including the manual generator’s `.env` test.

Changing into the project directory ensures that `Cargo.toml`, `src/main.rs`, and relative configuration paths refer to the target project rather than whichever directory happened to launch the command.

The generator `.env` is shell-sourced configuration. Document that it must be trusted and is not equivalent to the application’s simpler environment-file parser.

### Output correctness

Preserve `.generated/` reports and project-local transcripts, but correct candidate generation:

- Write the manual candidate to `$BASE_PATH/man/$appname.1.new`.
- Write completion candidates beneath `$BASE_PATH/completions`.
- Respect an existing `TAI_TRANSCRIPT_DIR` before applying generator defaults.
- Request raw roff or raw shell source, without Markdown fences or commentary.
- Ensure candidate output suppresses the user turn and contains no formatting decorations.

The commit-message workflow demonstrates the required distinction:

```text
CONTENT_ONLY
```

controls formatting, while:

```text
NO_USER_STDOUT
```

suppresses the echoed user prompt.

When both standard output and an artifact are used for generated content, explicitly apply:

```text
ENABLE_STDOUT,ENABLE_ARTIFACT,CONTENT_ONLY_STDOUT,CONTENT_ONLY_ARTIFACT,NO_USER_STDOUT,NO_USER_ARTIFACT
```

Apply these generation-specific controls after configurable controls so that a local formatting preference cannot accidentally contaminate an installable candidate.

Avoid the current unchecked `terminal-ai | tee` pipeline. Use Bash with `set -euo pipefail`, or capture output to a temporary file and check the command result explicitly.

Only replace a `.new` candidate after generation succeeds. Clean up temporary files on failure.

A successful process exit is not sufficient validation: the supplied application manual notes that some backend failures can still return zero. Check for empty output and review every candidate before promotion.

## Review and promotion workflow

Run the shared generators from the application checkout:

```bash
create-man
create-completions
```

Review candidates against the current implementation and `terminal-ai --help`.

Useful checks include:

```bash
man -l man/terminal-ai.1.new
fish -n completions/terminal-ai.fish.new
zsh -n completions/terminal-ai.zsh.new
bash -n completions/terminal-ai.bash.new
```

Run only checks for available files and shells. Syntax checks do not establish semantic correctness.

Promote each reviewed candidate explicitly:

```bash
mv man/terminal-ai.1.new man/terminal-ai.1
mv completions/terminal-ai.fish.new completions/terminal-ai.fish
```

Installation then copies the approved resources:

```bash
./install.sh
```

Keep `.generated/` and unreviewed `.new` outputs out of commits. Use the existing commit-message generator after staging the intended changes; it is not an installation dependency.

## Shell and manual discovery

Document discovery separately from copying files:

- **Fish:** the default user completion destination is automatically discovered. For the current session, source the installed file or start a new shell.
- **Bash:** lazy loading requires the `bash-completion` framework. Without it, users can source the installed completion explicitly.
- **Zsh:** users may need to add `$ZSH_COMPLETION_DIR` to `fpath` before running `compinit`.
- **Man:** verify discovery with `man -w terminal-ai`. If the local manual directory is not searched, document adding it to `MANPATH` while preserving the default search path.

Do not edit shell startup files or run `mandb` automatically.

## Future pacman packaging

Support staging now without adding a package definition yet:

```bash
PREFIX=/usr DESTDIR="$pkgdir" ./install.sh
```

With no destination overrides, resources should land under:

```text
$pkgdir/usr/bin/
$pkgdir/usr/share/man/man1/
$pkgdir/usr/share/fish/vendor_completions.d/
$pkgdir/usr/share/zsh/site-functions/
$pkgdir/usr/share/bash-completion/completions/
```

No files should be written to the invoking user’s home directory during this staged installation.

The eventual package should:

- Supply the built binary through `APP_BUILD_BIN` if needed.
- Include reviewed resources in its source/build inputs.
- Require no API credentials or generation dependencies.
- Let pacman own installed files and handle removal.
- Leave compression and distribution-specific hooks to the packaging tools.

## Acceptance criteria

- [ ] Existing binary and `tai-today` installation still works.
- [ ] Running the installer from another working directory works.
- [ ] Every optional resource installs independently when present.
- [ ] Missing optional resources do not fail installation.
- [ ] `.new` files are never installed.
- [ ] Failed copies return nonzero and do not print `installed`.
- [ ] Commands receive `0755`; resources receive `0644`.
- [ ] Paths containing spaces work.
- [ ] XDG user destinations and explicit destination overrides work.
- [ ] Reinstalling replaces reviewed resources without deleting unrelated files.
- [ ] Staging with `PREFIX=/usr` and `DESTDIR` writes only beneath the staging root.
- [ ] Installation works without a backend, credentials, generators, or installed shells.
- [ ] Generator commands use the caller’s repository and local environment.
- [ ] Failed generation does not replace an existing candidate.
- [ ] Available shell syntax checks and manual rendering checks pass.
- [ ] `docs/README.md` links to this plan, and user-facing installation guidance documents resource discovery.

