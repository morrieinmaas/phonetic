# Security

Phonetic is a desktop application. It opens no network ports. Its only outgoing request is the update
check, a read of GitHub's public releases API over HTTPS. The attack surface is parsing files you give it (WAV, calibration `.cal`, session JSON) and the Tauri IPC
bridge between the window and the Rust core.

## Reporting

Please report vulnerabilities privately to mo [at] traect [dot] dev instead of opening a public
issue, and include a way to reproduce. You will get an acknowledgement within a week. There is no
bug bounty: Phonetic is a free app maintained in spare time and comes with no guarantees.

## Supported versions

Only the latest release gets fixes.
