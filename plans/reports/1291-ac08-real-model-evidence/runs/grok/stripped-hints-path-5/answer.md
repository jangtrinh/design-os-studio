Goal: open `/workspace/dev/ac08/outside/brand.designos` with designos MCP only, add a frame named Hero, and save the result as `hero.designos` in the workspace.

I could not finish. `designos_open_file` refused the source path with `PATH_OUTSIDE_WORKSPACE`. Designos will not import a file outside its workspace. The user asked for MCP-only design work and forbade shell and file copying, so I did not copy `brand.designos` in.

Paths I tried:

- `/workspace/dev/ac08/outside/brand.designos` and `../outside/brand.designos` returned `PATH_OUTSIDE_WORKSPACE`.
- `outside/brand.designos` and `brand.designos` returned `INVALID_ARGUMENTS` (no such file inside the workspace).

The in-memory session is still a blank Untitled document with one empty page. I did not create a Hero frame on that blank document, and I did not write `hero.designos`.

To continue, put `brand.designos` inside the designos workspace, or point the designos workspace at a directory that already contains it. Then I can open it, add Hero, and save `hero.designos`.
