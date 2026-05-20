## Installation

### Windows GUI installer
Download `torn-war-report-<version>-setup.exe` (NSIS) or `.msi` and run it.

### macOS
Download the `.dmg`, mount it, and drag the app to Applications.
**Gatekeeper**: the build is unsigned. Right-click → Open the first time to bypass the warning.

### Linux CLI
```bash
tar xzf torn-war-report-<version>-x86_64-unknown-linux-gnu.tar.gz
./torn-war-report schema   # verify it works
```

## Laravel / server integration

Stable artifact URL pattern:
```
https://github.com/${{ github.repository }}/releases/download/<tag>/torn-war-report-<tag>-x86_64-unknown-linux-gnu.tar.gz
```
Latest pointer:
```
https://github.com/${{ github.repository }}/releases/latest/download/torn-war-report-x86_64-unknown-linux-gnu.tar.gz
```
