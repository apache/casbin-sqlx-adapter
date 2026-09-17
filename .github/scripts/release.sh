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

# Release helpers used by .github/workflows/release.yml. Run from the
# repository root.
#
#   release.sh meta <tag>                 tag -> version / is_rc / basename
#   release.sh set-version <version>      write the version into Cargo.toml
#   release.sh archive <version> <dir>    build the source tarball + .sha512
#   release.sh verify <version> <dir>     fetch the voted tarball from ASF,
#                                         check it, and compare it with git
#
# Cargo.toml carries the placeholder version 0.0.0. The real version only
# exists in the git tag (vX.Y.Z or vX.Y.Z-rcN) and is substituted here, so a
# release never needs a "bump version" commit.

set -euo pipefail

PLACEHOLDER='0.0.0'
ARTIFACT_PREFIX='apache-casbin-sqlx-adapter'
# Where promoted releases end up after the vote. Overridable for tests.
ASF_DIST_URL="${ASF_DIST_URL:-https://downloads.apache.org/incubator/casbin}"

fail() {
    echo "::error::$*" >&2
    exit 1
}

basename_for() {
    echo "${ARTIFACT_PREFIX}-$1-incubating-src"
}

require_version() {
    [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "'$1' is not a final version X.Y.Z"
}

# Substitute the placeholder in a Cargo.toml read from stdin.
substitute_version() {
    local version="$1" manifest
    manifest="$(cat)"
    local hits
    hits="$(grep -c "^version = \"$PLACEHOLDER\"$" <<<"$manifest" || true)"
    [[ "$hits" == 1 ]] || fail "Cargo.toml must contain exactly one 'version = \"$PLACEHOLDER\"' line (found $hits). The version comes from the git tag; do not hard-code it."
    sed "s/^version = \"$PLACEHOLDER\"$/version = \"$version\"/" <<<"$manifest"
}

# Print the id of a tree that is HEAD with the version written into
# Cargo.toml. Uses a scratch index, so neither the real index nor the working
# tree is touched.
source_tree() {
    local version="$1" blob
    blob="$(git show HEAD:Cargo.toml | substitute_version "$version" | git hash-object -w --stdin)"
    (
        export GIT_INDEX_FILE
        GIT_INDEX_FILE="$(mktemp)"
        git read-tree HEAD
        git update-index --cacheinfo "100644,$blob,Cargo.toml"
        git write-tree
        rm -f "$GIT_INDEX_FILE"
    )
}

cmd_meta() {
    local tag="${1:?usage: release.sh meta <tag>}"
    [[ "$tag" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)(-rc[1-9][0-9]*)?$ ]] || fail "Tag '$tag' is not vX.Y.Z or vX.Y.Z-rcN"
    local version="${BASH_REMATCH[1]}" is_rc=false
    [[ -n "${BASH_REMATCH[2]}" ]] && is_rc=true
    {
        echo "version=$version"
        echo "is_rc=$is_rc"
        echo "basename=$(basename_for "$version")"
    } >>"${GITHUB_OUTPUT:-/dev/stdout}"
}

cmd_set_version() {
    local version="${1:?usage: release.sh set-version <version>}"
    require_version "$version"
    local manifest
    manifest="$(substitute_version "$version" <Cargo.toml)"
    printf '%s
' "$manifest" >Cargo.toml
    echo "Cargo.toml version set to $version"
}

cmd_archive() {
    local version="${1:?usage: release.sh archive <version> <dir>}" dir="${2:?output dir required}"
    require_version "$version"
    local name tree
    name="$(basename_for "$version")"
    tree="$(source_tree "$version")"
    mkdir -p "$dir"
    # Tracked sources only, unpacking into a single top-level directory.
    git archive --format=tar.gz --prefix="$name/" -o "$dir/$name.tar.gz" "$tree"
    (cd "$dir" && sha512sum "$name.tar.gz" >"$name.tar.gz.sha512")
    echo "Built $dir/$name.tar.gz from tree $tree (HEAD $(git rev-parse HEAD) with version $version)"
}

cmd_verify() {
    local version="${1:?usage: release.sh verify <version> <dir>}" dir="${2:?output dir required}"
    require_version "$version"
    local name url tmp
    name="$(basename_for "$version")"
    url="$ASF_DIST_URL/sqlx-adapter-$version-incubating"
    [[ ! -e "$dir" ]] || [[ -z "$(ls -A "$dir")" ]] || fail "Output dir $dir already exists and is not empty"
    mkdir -p "$dir"
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064  # expand now: $tmp is local to this function
    trap "rm -rf '$tmp'" EXIT

    # 1. The voted files, exactly as promoted to the ASF release area.
    for suffix in '' .sha512 .asc; do
        curl -fsSL --retry 3 -o "$dir/$name.tar.gz$suffix" "$url/$name.tar.gz$suffix" \
            || fail "Cannot download $url/$name.tar.gz$suffix. Was the release promoted to the ASF dist area?"
    done
    curl -fsSL --retry 3 -o "$tmp/KEYS" "$ASF_DIST_URL/KEYS" || fail "Cannot download $ASF_DIST_URL/KEYS"

    # 2. Checksum and signature, against the project KEYS file only.
    (cd "$dir" && sha512sum -c "$name.tar.gz.sha512") || fail "SHA-512 mismatch for $name.tar.gz"
    (umask 077 && mkdir "$tmp/gnupg")
    gpg --batch --quiet --homedir "$tmp/gnupg" --import "$tmp/KEYS"
    gpg --batch --homedir "$tmp/gnupg" --verify "$dir/$name.tar.gz.asc" "$dir/$name.tar.gz" \
        || fail "GPG signature of $name.tar.gz does not verify against $ASF_DIST_URL/KEYS"

    # 3. The voted sources must be exactly what this tag points at.
    mkdir "$tmp/asf" "$tmp/git"
    tar -xzf "$dir/$name.tar.gz" -C "$tmp/asf"
    git archive --format=tar --prefix="$name/" "$(source_tree "$version")" | tar -x -C "$tmp/git"
    diff -r "$tmp/asf" "$tmp/git" || fail "The voted source archive differs from the sources at $(git rev-parse HEAD)"
    echo "Verified $name.tar.gz: checksum, signature and contents match $(git rev-parse HEAD)"
}

case "${1:-}" in
    meta) shift; cmd_meta "$@" ;;
    set-version) shift; cmd_set_version "$@" ;;
    archive) shift; cmd_archive "$@" ;;
    verify) shift; cmd_verify "$@" ;;
    *)
        echo "usage: release.sh meta <tag> | set-version <version> | archive <version> <dir> | verify <version> <dir>" >&2
        exit 2
        ;;
esac
