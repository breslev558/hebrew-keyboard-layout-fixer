# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org) (once the project is accepted; until then the public releases are not signed).

- Only the files built by GitHub Actions from this public repository (`.github/workflows/release.yml`) are signed; every release is approved by hand before signing.
- Roles: committer, reviewer and approver: [breslev558](https://github.com/breslev558) (multi-factor authentication enabled).
- Privacy: this program does not send any information to any networked system, except the optional daily update check, which only asks GitHub for the latest release of this project (no identifier, no telemetry). It can be switched off in the tray menu.
- System changes: the installer copies the program to the user's folder, adds it to the user's startup programs and to Windows "Installed apps"; Uninstall removes all of it.
