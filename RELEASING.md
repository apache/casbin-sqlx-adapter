<!--
Licensed to the Apache Software Foundation (ASF) under one
or more contributor license agreements. See the NOTICE file
distributed with this work for additional information
regarding copyright ownership. The ASF licenses this file
to you under the Apache License, Version 2.0 (the
"License"); you may not use this file except in compliance
with the License. You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing,
software distributed under the License is distributed on an
"AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, either express or implied. See the License for the
specific language governing permissions and limitations
under the License.
-->

# Releasing SQLx Adapter

Complete version, dependency, licensing, packaging and build/test preparation
in one PR. After merge, freeze the actual commit, check its CI and verify its
source archive. Review LICENSE, NOTICE, DISCLAIMER and every RAT exclusion.
Run `python3 -m unittest discover -s .github/scripts -p 'test_*.py'` for the
publication checks (Python 3.11+). A green build does not replace release votes.

## RC and votes

At the reviewed commit, create `vX.Y.Z-rcN` with the plain `X.Y.Z` already in
Cargo.toml. The workflow creates a GitHub prerelease containing
`apache-casbin-sqlx-adapter-X.Y.Z-incubating-src.tar.gz` and its `.sha512`.
RC jobs do not load registry/signing credentials or publish registry packages.

Download, inspect and independently build the exact generated archive. Sign
it to produce `.asc`, then stage all three files in
`https://dist.apache.org/repos/dist/dev/incubator/casbin/sqlx-adapter-X.Y.Z-incubating-rcN/`.
Confirm the signer is in the official Casbin KEYS file and that the publisher
has ASF distribution write access. Verify public downloads, checksum and
signature before conducting the applicable project and Incubator votes.
Never overwrite a staged candidate: changed bytes require a new RC number.

## Final release

After the applicable votes pass, promote the same three files to the ASF
release directory `incubator/casbin/sqlx-adapter-X.Y.Z-incubating/`. Wait until
all files are available under `https://downloads.apache.org/incubator/casbin/`.
Create `vX.Y.Z` at the reviewed RC commit. The final workflow downloads these
originals, verifies SHA-512 and GPG against Casbin KEYS, and compares their file
contents with Git. It does not rebuild or replace the voted source archive.

GitHub Release creation refuses an existing Release. Registry publication
runs only after the GitHub Release succeeds, and independently verifies ASF
originals before loading publication credentials. On a registry job failure,
check the registry version first, then use GitHub Actions to rerun only failed
jobs if publication did not complete. Do not delete/recreate voted candidates
or rerun all jobs to replace existing assets. Keep vote results, artifact
checks and publication evidence together; send announcements as authorized.

Crates.io publication uses the repository secret `CARGO_REGISTRY_TOKEN`.
The token must authorize publishing the `sqlx-adapter` crate.

## Build from an extraction

Use Rust 1.94 or newer and build one driver at a time. For example:

```sh
cargo build --no-default-features --features sqlite,runtime-tokio-rustls
DATABASE_URL=sqlite:casbin.db cargo test --no-default-features --features sqlite,runtime-tokio-rustls
```

Create the disposable SQLite database before testing. For PostgreSQL and
MySQL, create the policy table using the SQL in `.github/workflows/ci.yml`
before running the parallel tests, as CI does. Concurrent initialization of
an empty PostgreSQL database can otherwise race while creating the table.
Repeat tests for PostgreSQL and MySQL with their own test
`DATABASE_URL`, and for the supported Tokio/async-std and native-tls/rustls
feature combinations. The CI workflow records the full matrix. Keep the
original archive intact and test a disposable extraction.
