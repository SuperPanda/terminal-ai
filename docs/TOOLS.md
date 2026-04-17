# Tools

A collection of tools put together to solve problems, and are subject to change, and will require syncing.

## Overview

[tools]
 ├─fix-tools.sh                 : Runs shellcheck over tools/ -> Sends to AI -> puts report in .generated/fix-<UUID v7>
 ├─fix.sh                       : Runs clippy over  -> Sends to AI -> puts report in .generated/shellcheck-<UUID v7>
 ├─generate-commit-msg.sh       : Pipes `git diff --cached` -> Sends to AI -> puts commit message in ./generated/change-<UUID v7
 ├─list-generated-files.sh      : Outputs a (datetime, prefix, path) tsv for commandline operations
 ├─show-fix.sh                  : Takes a `GENERATED_FILE_PREFIX` *environment variable, and shows the latest one (which are exported by fix-tools and fix)
 └─view-last-commit-msg.sh      : Prints the most recent generated commit


> **Notes:** 
>
> The `show-fix.sh` tool 'mdless' unless `--pipe` is provided as an argument.
>
> The `help` prefix was *fix*

> **Fix Path:**
>
>   The fix tools use the `./target/release/terminal-ai` as that is the more stab release. 

> **Release Path:**
>
>  The `generate-commit-msg` should use the  `./target/debug/terminal-ai` path,
>  as this will highlight major issues while dog fooding
>
> Testing will be put into place soon.

Manual invocations, should be set to output via stdout or file to `.generated/assistant-$(uuidgen -7)`



## Workflows


### Git Commits


1. GitUI to stage a unit or self-contained commit.
2. Use `./tools/generate-commit-msg.sh`
3. Review that it works using `./tool`


## Appendix: Links

[Tool: fix tools]: tools/fix-tools.sh
[Tool: fix code]: tools/fix.sh
[Tool: commit message generator]: tools/generate-commit-msg.sh
[Tool: show latest fix]: tool/show-fix.sh
[Tool: view generated commit message]: tools/view-last-commit-msg.sh 
