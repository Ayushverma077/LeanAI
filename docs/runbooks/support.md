# Support and incident runbook

## Intake

Ask for Settings → Diagnostics (versions, platform, schema, capabilities). It
contains no paths, source or credentials, so it is safe to paste into a ticket.

**Never ask a user to send their bundle, their project, or a screenshot of the
preview.** The bundle is their source code. Ask for the manifest instead: it has
the fingerprint, revision, selection and options with no file contents.

## Triage

| Severity | Definition | Response |
| --- | --- | --- |
| S1 | Data loss, or content leaves the device without consent | Stop the rollout, advisory within 24 h |
| S2 | A core flow is broken (scan, bundle, export) on a supported platform | Fix in the next patch |
| S3 | Wrong or misleading information shown (bad exclusion reason, wrong freshness) | Fix in the next minor; treat as correctness, not cosmetics |
| S4 | Cosmetic or enhancement | Backlog |

A secret-scan false negative is **not** an S1 by itself: the scanner is
documented as a review aid. A missing *exclusion* of a known credential path is
an S1, because the policy is the boundary.

## Common diagnoses

| Report | First checks |
| --- | --- |
| "A file is missing" | Ignore precedence; is it listed greyed with a reason? Is the scan truncated? |
| "The token count is wrong" | It is an OpenAI-family estimate — confirm which model they compared against |
| "The bundle differs between machines" | Line-ending normalisation on? Same revision, selection and options? Compare the two manifests |
| "Context is wrong" | Check the section's freshness badge and its limitations; open the cited source |
| "It is slow" | File count, largest directory, whether `node_modules`-style directories are being reached through a symlink |
| "It will not open my project" | Permissions on the directory; on macOS, whether folder access was granted |

## Security disclosure

Private advisory or email to the maintainers. Acknowledge within 3 working days.
Do not ask for source or credentials in the report. Fix, add a regression test,
release, then disclose with credit.

## Incident record

Every S1/S2 gets: timeline, affected versions, user impact, root cause, the test
that now covers it, and what would have caught it earlier.
