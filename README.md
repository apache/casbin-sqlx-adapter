<!--
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
-->

# sqlx-adapter

[![Crates.io](https://img.shields.io/crates/v/sqlx-adapter.svg)](https://crates.io/crates/sqlx-adapter)
[![Docs](https://docs.rs/sqlx-adapter/badge.svg)](https://docs.rs/sqlx-adapter)
[![CI](https://github.com/apache/casbin-sqlx-adapter/actions/workflows/ci.yml/badge.svg)](https://github.com/apache/casbin-sqlx-adapter/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/apache/casbin-sqlx-adapter/branch/master/graph/badge.svg)](https://codecov.io/gh/apache/casbin-sqlx-adapter)

sqlx-adapter is the [Sqlx](https://github.com/launchbadge/sqlx) adapter for [casbin-rs](https://github.com/apache/casbin-rs). With this library, Casbin can load policy from Sqlx supported database or save policy to it with fully asynchronous support.

Based on [Sqlx](https://github.com/launchbadge/sqlx), The current supported databases are:

- [MySQL](https://www.mysql.com/)
- [PostgreSQL](https://github.com/lib/pq)
- [SQLite](https://www.sqlite.org)

## Notice

In order to unify the database table name in Casbin ecosystem, we decide to use `casbin_rule` instead of `casbin_rules` from version `0.4.0`. If you are using old version `sqlx-adapter` in your production environment, please use following command and update `sqlx-adapter` version:

````SQL
# MySQL & PostgreSQL & SQLite
ALTER TABLE casbin_rules RENAME TO casbin_rule;
````

## Install

Building the SQLx 0.9 source requires Rust 1.94 or newer. Compile each database
driver separately; database integration tests additionally require that
database to be running and `DATABASE_URL` to be configured.

Add the following to `Cargo.toml`:

For MySQL:

```toml
sqlx-adapter = { version = "1.9.0", default-features = false, features = ["mysql", "runtime-tokio", "tls-native-tls"]}
tokio = { version = "1.1.1", features = ["macros"] }
```

For PostgreSQL:

```toml
sqlx-adapter = { version = "1.9.0", default-features = false, features = ["postgres", "runtime-tokio", "tls-native-tls"]}
tokio = { version = "1.1.1", features = ["macros"] }
```

For SQLite:

```toml
sqlx-adapter = { version = "1.9.0", default-features = false, features = ["sqlite", "runtime-tokio", "tls-native-tls"]}
tokio = { version = "1.1.1", features = ["macros"] }
```

**Warning**: `tokio v1.0` or later is supported from `sqlx-adapter v0.4.0`, we recommend that you upgrade the relevant components to ensure that they work properly. The last version that supports `tokio v0.2` is `sqlx-adapter v0.3.0` , you can choose according to your needs.

## Configure

**No database is needed to build this crate.** All queries are executed at runtime, so `cargo build` works offline and you never have to set `DATABASE_URL` or run `cargo sqlx prepare` in your own project. The steps below are only about the database your application talks to at runtime (and about running this repository's own test suite).

1. Set up database environment
   
    One convenient option is using docker to get your database environment ready:
    
    ```bash
    #!/bin/bash

    DIS=$(lsb_release -is)

    command -v docker > /dev/null 2>&1 || {
        echo "Please install docker before running this script." && exit 1;
    }

    if [ $DIS == "Ubuntu" ] || [ $DIS == "LinuxMint" ]; then
        sudo apt install -y \
            libpq-dev \
            libmysqlclient-dev \
            postgresql-client \
            mysql-client-core;

    elif [ $DIS == "Deepin" ]; then
        sudo apt install -y \
            libpq-dev \
            libmysql++-dev \
            mysql-client \
            postgresql-client;
    elif [ $DIS == "ArchLinux" ] || [ $DIS == "ManjaroLinux" ]; then
        sudo pacman -S libmysqlclient \
            postgresql-libs \
            mysql-clients \;
    else
        echo "Unsupported system: $DIS" && exit 1;
    fi

    docker run -itd \
        --restart always \
        -e POSTGRES_USER=casbin_rs \
        -e POSTGRES_PASSWORD=casbin_rs \
        -e POSTGRES_DB=casbin \
        -p 5432:5432 \
        -v /srv/docker/postgresql:/var/lib/postgresql \
        postgres:11;

    docker run -itd \
        --restart always \
        -e MYSQL_ALLOW_EMPTY_PASSWORD=yes \
        -e MYSQL_USER=casbin_rs \
        -e MYSQL_PASSWORD=casbin_rs \
        -e MYSQL_DATABASE=casbin \
        -p 3306:3306 \
        -v /srv/docker/mysql:/var/lib/mysql \
        mysql:8 \
        --default-authentication-plugin=mysql_native_password;

    ```

2. Create table `casbin_rule`

    `SqlxAdapter` issues `CREATE TABLE IF NOT EXISTS` on startup, so this is optional — do it yourself only if your database user is not allowed to create tables.

    ```bash
    # PostgreSQL
    psql postgres://casbin_rs:casbin_rs@127.0.0.1:5432/casbin -c "CREATE TABLE IF NOT EXISTS casbin_rule (
        id SERIAL PRIMARY KEY,
        ptype VARCHAR NOT NULL,
        v0 VARCHAR NOT NULL,
        v1 VARCHAR NOT NULL,
        v2 VARCHAR NOT NULL,
        v3 VARCHAR NOT NULL,
        v4 VARCHAR NOT NULL,
        v5 VARCHAR NOT NULL,
        CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
        );"

    # MySQL
    mysql -h 127.0.0.1 -u casbin_rs -pcasbin_rs casbin 

    CREATE TABLE IF NOT EXISTS casbin_rule (
        id INT NOT NULL AUTO_INCREMENT,
        ptype VARCHAR(12) NOT NULL,
        v0 VARCHAR(128) NOT NULL,
        v1 VARCHAR(128) NOT NULL,
        v2 VARCHAR(128) NOT NULL,
        v3 VARCHAR(128) NOT NULL,
        v4 VARCHAR(128) NOT NULL,
        v5 VARCHAR(128) NOT NULL,
        PRIMARY KEY(id),
        CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8;
   
   # SQLite
   touch casbin.db
   
   sqlite3 casbin.db -cmd "CREATE TABLE IF NOT EXISTS casbin_rule (
       id INTEGER PRIMARY KEY,
       ptype VARCHAR(12) NOT NULL,
       v0 VARCHAR(128) NOT NULL,
       v1 VARCHAR(128) NOT NULL,
       v2 VARCHAR(128) NOT NULL,
       v3 VARCHAR(128) NOT NULL,
       v4 VARCHAR(128) NOT NULL,
       v5 VARCHAR(128) NOT NULL,
       CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
       );"
    ```

3. Configure `env` (only needed to run this repository's tests)

    Rename `sample.env` to `.env` and put `DATABASE_URL`, `POOL_SIZE`   inside

    ```bash
    DATABASE_URL=postgres://casbin_rs:casbin_rs@localhost:5432/casbin
    # DATABASE_URL=mysql://casbin_rs:casbin_rs@localhost:3306/casbin
    # DATABASE_URL=sqlite:casbin.db
    POOL_SIZE=8
    ```

    Or you can export `DATABASE_URL`, `POOL_SIZE`

    ```bash
    export DATABASE_URL=postgres://casbin_rs:casbin_rs@localhost:5432/casbin
    export POOL_SIZE=8
    ```


## Example

```rust
use sqlx_adapter::casbin::prelude::*;
use sqlx_adapter::casbin::Result;
use sqlx_adapter::SqlxAdapter;

#[tokio::main]
async fn main() -> Result<()> {
    let m = DefaultModel::from_file("examples/rbac_model.conf").await?;
    
    let a = SqlxAdapter::new("postgres://casbin_rs:casbin_rs@127.0.0.1:5432/casbin", 8).await?;
    let mut e = Enforcer::new(m, a).await?;
    
    Ok(())
}

```

## Features

- `postgres`
- `mysql`
- `sqlite`
- `runtime-tokio`
- `runtime-async-std`
- `tls-native-tls`
- `tls-rustls`

The legacy combined runtime/TLS features remain available as backward-compatible aliases:

- `runtime-tokio-native-tls`
- `runtime-tokio-rustls`
- `runtime-async-std-native-tls`
- `runtime-async-std-rustls`

*Attention*: `postgres`, `mysql`, `sqlite` are mutual exclusive which means that you can only activate one of them.
