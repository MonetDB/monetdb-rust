How to release a new version of monetdb-rust
============================================

To prepare for the release, first make sure [CHANGELOG.md] contains all relevant
changes, the documentation is tidy, CI is clean, etc. etc. Script
`checklicense.py` can be used to check if all files contain a copyright notice.

Note: there is no need to add a new section header to [CHANGELOG.md], the
scripts will do that.

Then use [cargo release] to create a new section in in [CHANGELOG.md], bump the
version number in Cargo.toml, create a Git tag, publish the crate on [crates.io]
and push the tag to GitHub. This can be done with a single command,  e.g.,
`cargo release minor`, but you can also let it perform the steps separately. See
Section [Steps](#steps) below.

Note: until we are a bit more experienced, the automated 'publish' and 'push' steps have been disabled and must be performed manually.

When this is done, use `cargo release alpha` to switch the main branch back to a
development version number. For example, after releasing version 0.7.10, `cargo
release alpha` bumps to version 0.7.11-alpha.1. Push this commit to GitHub too,
but do not tag it.

Also, you need to manually create a GitHub release from the tag,
with text copy-pasted from [CHANGELOG.md].
I don't think [cargo release] can automate that.


Steps
-----

The configuration file for [cargo release] is [release.toml].
When in doubt, check the [documentation][cargo release docs].

When you run for example `cargo release minor`, [cargo release] will perform all
steps below. The command to run individual steps is noted in the stepTo execute
individual steps, write `cargo release version <version>`, `cargo release
replace`, etc. You can also perform these actions by hand if that feels safer.

1. `cargo release version <major|minor|patch|alpha>`. Bumps the version number in
   Cargo.toml.

2. `cargo release replace`. Adds a section for this version to [CHANGELOG.md], containing
   the items that used to be in section "NEXTVERSION".

3. `cargo release hook`. Runs the pre-release hooks configured in [release.toml].
   At the time of writing, this checks the copyright messages.

4. `cargo release commit`. Commits the changed version number and the updated
   CHANGELOG.md. The commit message is configured in [release.toml].

5. `cargo release publish`. Currently disabled in [release.toml], runs `cargo
   publish`. Before running this, set env var `CARGO_REGISTRY_TOKEN`
   to a token obtained from `cargo login` or https://crates.io/me.

6. `cargo release tag`. Creates a tag `vMAJOR.MINOR.PATCH`.

7. `cargo release push`. Pushes the commits to GitHub. Currently disabled in
   [release.toml].


[cargo release]: https://github.com/crate-ci/cargo-release
[cargo release docs]: https://github.com/crate-ci/cargo-release/blob/master/docs/reference.md
[CHANGELOG.md]: ../CHANGELOG.md
[crates.io]: https://crates.io
[release.toml]: ../release.toml