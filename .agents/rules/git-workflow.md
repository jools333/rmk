# Mandatory Git Workflow: Commit & Push After Edits

Whenever you make ANY changes in this project (Rust source code in `src/`, configuration in `keyboard.toml`, keymaps, documentation `README.md` / `AGENTS.md`, or build scripts):

1. **Rebuild binaries**: If code or keyboard configuration was changed, compile release firmware with `./build.sh` (or `build.ps1`) to ensure `dist/` is updated.
2. **Commit changes**: Stage modified files and make a clean, descriptive Git commit following conventional commit standards (`feat(...)`, `fix(...)`, `docs(...)`, `chore(...)`).
3. **Push immediately**: Always push your commits to `origin main` via `git push origin main` before completing the task.

**CRITICAL**: Leaving uncommitted or unpushed changes at the end of a session is strictly forbidden.
