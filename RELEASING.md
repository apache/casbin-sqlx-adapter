<!--
Licensed to the Apache Software Foundation (ASF) under one
or more contributor license agreements.  See the NOTICE file
distributed with this work for additional information
regarding copyright ownership.  The ASF licenses this file
to you under the Apache License, Version 2.0 (the
"License"); you may not use this file except in compliance
with the License.  You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing,
software distributed under the License is distributed on an
"AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, either express or implied.  See the License for the
specific language governing permissions and limitations
under the License.
-->

# Releasing

There is no version to bump. `Cargo.toml` always says `version = "0.0.0"`;
the real version comes from the git tag and is written in by
[`.github/scripts/release.sh`](.github/scripts/release.sh) when the source
tarball is built and when the crate is published. A release is therefore just
two tags pushed at the same commit, with the Apache vote in between.

The official release is the source tarball voted on by the PPMC and the
Incubator PMC. The GitHub release and the crates.io package are conveniences
built from exactly those bytes.

## 1. Release candidate

Pick the commit to release (CI green on `master`) and push an RC tag:

```bash
git tag v1.9.0-rc1 <commit> && git push origin v1.9.0-rc1
```

The `Release` workflow creates a GitHub **pre-release** carrying

- `apache-casbin-sqlx-adapter-1.9.0-incubating-src.tar.gz`
- `apache-casbin-sqlx-adapter-1.9.0-incubating-src.tar.gz.sha512`

The tarball is `git archive` of the tagged commit with the version written
into `Cargo.toml`. Nothing is signed and nothing reaches crates.io.

Download both files, check the tarball builds and tests from a clean
extraction, then sign it with the key you have in the Casbin
[KEYS](https://downloads.apache.org/incubator/casbin/KEYS) file:

```bash
gpg --armor --detach-sign apache-casbin-sqlx-adapter-1.9.0-incubating-src.tar.gz
```

Stage the three files (`.tar.gz`, `.sha512`, `.asc`) under
`https://dist.apache.org/repos/dist/dev/incubator/casbin/sqlx-adapter-1.9.0-incubating-rc1/`
and start the vote. If anything has to change, fix it on `master`, push
`v1.9.0-rc2` at the new commit and stage a new candidate directory; never
overwrite staged files.

## 2. Final release

After the votes pass, `svn mv` the candidate directory to
`https://dist.apache.org/repos/dist/release/incubator/casbin/sqlx-adapter-1.9.0-incubating/`
and wait until it shows up at
`https://downloads.apache.org/incubator/casbin/sqlx-adapter-1.9.0-incubating/`.

Then push the final tag **at the same commit as the voted RC**:

```bash
git tag v1.9.0 <commit> && git push origin v1.9.0
```

The workflow does not rebuild anything. It downloads the three promoted files,
checks the SHA-512, checks the GPG signature against the Casbin `KEYS` file,
checks that the tarball's contents are identical to the tagged commit, and
only then creates the GitHub release with those files and runs
`cargo publish` (using the repository secret `CARGO_REGISTRY_TOKEN`). Any
mismatch fails the workflow before anything is published.

`gh release create` refuses to touch an existing release, so re-running the
workflow can never replace published assets. If only the crates.io job failed,
re-run just that job.

## Checks you can run locally

```bash
.github/scripts/test-release.sh      # exercises release.sh end to end, offline
.github/scripts/release.sh archive 1.9.0 /tmp/dist   # build the tarball yourself
```

License headers are checked by the `License Headers` CI job with Apache RAT;
files that cannot carry a header are listed in [`.rat-excludes`](.rat-excludes).
