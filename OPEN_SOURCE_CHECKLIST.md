# Open-source readiness checklist

This checklist records the repository checks completed before publishing Oxitone on GitHub. It is intentionally
kept in the repository so future releases can repeat the review.

## Legal and metadata

- [x] Root license replaced with the unmodified Mozilla Public License 2.0 (`LICENSE`, SPDX `MPL-2.0`).
- [x] Cargo workspace and every npm package declare `MPL-2.0`.
- [x] README states the license, pre-1.0 status, supported platform and current release gaps.
- [x] Contribution terms and development expectations are documented in `CONTRIBUTING.md`.
- [ ] Review third-party dependency notices and publish an SBOM as part of the release workflow.
- [ ] Confirm the final copyright holder and GitHub organization ownership before the first tagged release.

## Repository hygiene

- [x] Generated bindings, build output, `node_modules`, native binaries and `target` are ignored by `.gitignore`.
- [x] No tracked `.env`, credential, token or private-key filenames were found in the pre-publish scan.
- [x] No tracked files outside ignored build directories exceed the repository size budget.
- [x] Logo source and generation metadata are stored under `docs/assets/`; no API token is committed.
- [ ] Enable GitHub secret scanning, Dependabot and branch protection after creating the remote repository.

## Verification

- [x] Existing macOS CI workflow covers locked install, build, schema drift, lint, typecheck, Rust tests, browser
      no-device audio checks, examples and focused benchmarks on arm64 and x64.
- [ ] Run the complete local release checks on a clean checkout immediately before the first public push.
- [ ] Perform a clean npm/package dry run and validate that no generated artifacts or private paths are included.
- [ ] Complete fuzzing, sanitizer, endurance, SBOM, signing and notarization gates tracked in the implementation
      status document before calling a release stable.

The unchecked items are release gates, not hidden compatibility promises. The project remains pre-1.0 until they are
closed and the implementation status document is updated with evidence.
