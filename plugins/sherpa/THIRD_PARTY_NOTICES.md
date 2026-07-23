# Third-party notices

## RemCTL

Sherpa can fetch and install RemCTL 1.5.1 from `https://github.com/viticci/remctl.git` at commit `eb75c451eab006218204bb78379917f3414fc6e3`.

RemCTL is copyright its contributors and is distributed under the MIT License. Its source and license remain available in the upstream repository. The reviewed [full MIT license text](third_party/remctl/LICENSE) is included with Sherpa and installed beside the managed runtime notices. Sherpa verifies the pinned commit and license, runs the upstream installer only in a temporary staging root, and exposes the result solely through `sherpa planner reminders`. Upstream command aliases, completions, and staging configuration are not copied. Running `sherpa planner reminders onboard` later may create the dependency's private configuration directory.

## Optional message readers

`kakaocli` and `imsg` are optional external executables. They are not bundled or installed automatically. Review their source, license, and permissions before enabling the corresponding message source.
