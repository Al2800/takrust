# Security Policy

## Supported Versions

Security fixes are applied to the `main` branch.

At this stage of the project, release tags are still stabilizing. If you run a
pinned commit, update to the latest `main` before reporting a potential issue.

## Reporting a Vulnerability

Please do **not** open public GitHub issues for suspected vulnerabilities.

Report privately to the maintainers with:

- affected component(s) and file paths
- reproduction steps and expected vs actual behavior
- impact assessment (confidentiality/integrity/availability)
- any proposed mitigation

If you do not have a private reporting path configured, open a minimal public
issue requesting secure contact details and do not include exploit details.

## Response Expectations

Maintainers will triage reports and reply with:

- severity and scope assessment
- mitigation or patch plan
- disclosure coordination timeline when applicable

## Scope Notes

Priority surfaces for this repository include:

- transport parsing and framing boundaries
- cryptographic/provider configuration
- replay/record integrity and deterministic gate logic
- admin/control-plane endpoint exposure
