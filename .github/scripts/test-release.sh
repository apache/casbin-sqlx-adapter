#!/usr/bin/env bash

# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

# End-to-end test for release.sh against a throwaway git repository, a
# throwaway GPG key and a fake ASF dist area served over file://. Nothing
# touches the network or the real repository. Needs git, gpg, curl, tar,
# sha512sum.

set -euo pipefail

RELEASE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

VERSION=1.2.3
NAME="apache-casbin-sqlx-adapter-$VERSION-incubating-src"
ASF="$WORK/asf"
RELEASE_DIR="$ASF/sqlx-adapter-$VERSION-incubating"

# curl wants file:///C:/... on Windows and file:///tmp/... elsewhere.
file_url() {
    local p
    p="$(cygpath -m "$1" 2>/dev/null || printf '%s' "$1")"
    case "$p" in /*) echo "file://$p" ;; *) echo "file:///$p" ;; esac
}

pass() { echo "ok   - $*"; }
expect_fail() {
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then
        echo "FAIL - $what: expected failure, but it succeeded" >&2
        exit 1
    fi
    pass "$what (rejected)"
}

commit() {
    git -c user.name=test -c user.email=test@example.invalid commit -q --allow-empty -am "$1"
}

# --- fixture repository ----------------------------------------------------
mkdir repo && cd repo
git init -q
printf '[package]\nname = "sqlx-adapter"\nversion = "0.0.0"\nedition = "2018"\n' >Cargo.toml
echo 'fixture' >LICENSE
git add . && commit 'fixture'

# --- meta --------------------------------------------------------------------
out="$WORK/meta"
GITHUB_OUTPUT="$out" "$RELEASE" meta v1.2.3
grep -qx "version=1.2.3" "$out" && grep -qx "is_rc=false" "$out" && grep -qx "basename=$NAME" "$out"
pass "meta v1.2.3"
: >"$out"
GITHUB_OUTPUT="$out" "$RELEASE" meta v1.2.3-rc7
grep -qx "version=1.2.3" "$out" && grep -qx "is_rc=true" "$out" && grep -qx "basename=$NAME" "$out"
pass "meta v1.2.3-rc7 (same artifact name as the final release)"
for tag in 1.2.3 v1.2 v1.2.3-rc0 v1.2.3-rc v1.2.3-beta1 v1.2.3rc1 master; do
    expect_fail "meta $tag" "$RELEASE" meta "$tag"
done

# --- set-version -------------------------------------------------------------
"$RELEASE" set-version "$VERSION" >/dev/null
grep -qx "version = \"$VERSION\"" Cargo.toml && pass "set-version writes $VERSION"
expect_fail "set-version twice (placeholder already gone)" "$RELEASE" set-version "$VERSION"
expect_fail "set-version with rc suffix" "$RELEASE" set-version "$VERSION-rc1"
git checkout -q -- Cargo.toml

# --- archive -----------------------------------------------------------------
"$RELEASE" archive "$VERSION" "$WORK/dist" >/dev/null
(cd "$WORK/dist" && sha512sum -c --quiet "$NAME.tar.gz.sha512")
pass "archive produces tarball and matching .sha512"
tar -tzf "$WORK/dist/$NAME.tar.gz" | grep -vq "^$NAME/" && { echo "FAIL - entries outside $NAME/" >&2; exit 1; }
tar -xzf "$WORK/dist/$NAME.tar.gz" -C "$WORK" "$NAME/Cargo.toml"
grep -qx "version = \"$VERSION\"" "$WORK/$NAME/Cargo.toml" && pass "archive carries the tag version"
grep -qx 'version = "0.0.0"' Cargo.toml && [[ -z "$(git status --porcelain)" ]] && pass "archive leaves the working tree untouched"
expect_fail "archive with rc suffix" "$RELEASE" archive "$VERSION-rc1" "$WORK/dist-rc"

# --- fake ASF dist area -------------------------------------------------------
(umask 077 && mkdir "$WORK/gnupg" "$WORK/gnupg-other")
gpg --batch --quiet --homedir "$WORK/gnupg" --passphrase '' --quick-generate-key 'Release Manager <rm@example.invalid>' default default never
gpg --batch --quiet --homedir "$WORK/gnupg-other" --passphrase '' --quick-generate-key 'Someone Else <other@example.invalid>' default default never
mkdir -p "$RELEASE_DIR"
gpg --batch --quiet --homedir "$WORK/gnupg" --armor --export >"$ASF/KEYS"
cp "$WORK/dist/$NAME.tar.gz" "$WORK/dist/$NAME.tar.gz.sha512" "$RELEASE_DIR/"
sign() { gpg --batch --quiet --homedir "$1" --armor --yes --detach-sign -o "$RELEASE_DIR/$NAME.tar.gz.asc" "$RELEASE_DIR/$NAME.tar.gz"; }
sign "$WORK/gnupg"
export ASF_DIST_URL
ASF_DIST_URL="$(file_url "$ASF")"

# --- verify: happy path -------------------------------------------------------
"$RELEASE" verify "$VERSION" "$WORK/verified"
for f in "$NAME.tar.gz" "$NAME.tar.gz.sha512" "$NAME.tar.gz.asc"; do
    cmp -s "$WORK/verified/$f" "$RELEASE_DIR/$f"
done
pass "verify accepts the promoted files and keeps them byte for byte"
expect_fail "verify into a non-empty dir" "$RELEASE" verify "$VERSION" "$WORK/verified"

# --- verify: rejections -------------------------------------------------------
expect_fail "verify with rc suffix" "$RELEASE" verify "$VERSION-rc1" "$WORK/v0"
expect_fail "verify unknown version (nothing promoted)" "$RELEASE" verify 9.9.9 "$WORK/v1"

mv "$RELEASE_DIR/$NAME.tar.gz.asc" "$WORK/asc.bak"
expect_fail "verify without .asc" "$RELEASE" verify "$VERSION" "$WORK/v2"
mv "$WORK/asc.bak" "$RELEASE_DIR/$NAME.tar.gz.asc"

sign "$WORK/gnupg-other"
expect_fail "verify with a signature from a key not in KEYS" "$RELEASE" verify "$VERSION" "$WORK/v3"
sign "$WORK/gnupg"

cp "$RELEASE_DIR/$NAME.tar.gz" "$WORK/tgz.bak"
printf 'x' >>"$RELEASE_DIR/$NAME.tar.gz"
expect_fail "verify with a tampered tarball" "$RELEASE" verify "$VERSION" "$WORK/v4"
cp "$WORK/tgz.bak" "$RELEASE_DIR/$NAME.tar.gz"

# Valid checksum and signature, but the tag no longer points at the voted
# sources: this is the case the download step alone cannot catch.
echo 'changed after the vote' >>LICENSE
commit 'post-vote change'
expect_fail "verify when the tag differs from the voted sources" "$RELEASE" verify "$VERSION" "$WORK/v5"
git reset -q --hard HEAD~1
"$RELEASE" verify "$VERSION" "$WORK/v6" >/dev/null 2>&1 && pass "verify passes again at the voted commit"

echo "all release.sh checks passed"
