#!/bin/bash
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements. See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership. The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License. You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied. See the License for the
# specific language governing permissions and limitations
# under the License.

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
