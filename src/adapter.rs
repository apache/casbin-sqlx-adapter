use async_trait::async_trait;
use casbin::{error::AdapterError, Adapter, Error as CasbinError, Filter, Model, Result};
use dotenvy::dotenv;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{error::*, models::*};

use crate::actions as adapter;

#[cfg(feature = "mysql")]
use sqlx::mysql::MySqlPoolOptions;
#[cfg(feature = "postgres")]
use sqlx::postgres::PgPoolOptions;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePoolOptions;

#[derive(Clone)]
pub struct SqlxAdapter {
    pool: adapter::ConnectionPool,
    is_filtered: Arc<AtomicBool>,
}

//pub const TABLE_NAME: &str = "casbin_rule";

impl<'a> SqlxAdapter {
    pub async fn new<U: Into<String>>(url: U, pool_size: u32) -> Result<Self> {
        dotenv().ok();

        #[cfg(feature = "postgres")]
        let pool = PgPoolOptions::new()
            .max_connections(pool_size)
            .connect(&url.into())
            .await
            .map_err(|err| CasbinError::from(AdapterError(Box::new(Error::SqlxError(err)))))?;

        #[cfg(feature = "mysql")]
        let pool = MySqlPoolOptions::new()
            .max_connections(pool_size)
            .connect(&url.into())
            .await
            .map_err(|err| CasbinError::from(AdapterError(Box::new(Error::SqlxError(err)))))?;

        #[cfg(feature = "sqlite")]
        let pool = SqlitePoolOptions::new()
            .max_connections(pool_size)
            .connect(&url.into())
            .await
            .map_err(|err| CasbinError::from(AdapterError(Box::new(Error::SqlxError(err)))))?;

        adapter::new(&pool).await.map(|_| Self {
            pool,
            is_filtered: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn new_with_pool(pool: adapter::ConnectionPool) -> Result<Self> {
        adapter::new(&pool).await.map(|_| Self {
            pool,
            is_filtered: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn save_policy_line(
        &self,
        ptype: &'a str,
        rule: &'a [String],
    ) -> Option<NewCasbinRule<'a>> {
        if ptype.trim().is_empty() || rule.is_empty() {
            return None;
        }

        let mut new_rule = NewCasbinRule {
            ptype,
            v0: "",
            v1: "",
            v2: "",
            v3: "",
            v4: "",
            v5: "",
        };

        new_rule.v0 = &rule[0];

        if rule.len() > 1 {
            new_rule.v1 = &rule[1];
        }

        if rule.len() > 2 {
            new_rule.v2 = &rule[2];
        }

        if rule.len() > 3 {
            new_rule.v3 = &rule[3];
        }

        if rule.len() > 4 {
            new_rule.v4 = &rule[4];
        }

        if rule.len() > 5 {
            new_rule.v5 = &rule[5];
        }

        Some(new_rule)
    }

    pub(crate) fn load_policy_line(&self, casbin_rule: &CasbinRule) -> Option<Vec<String>> {
        if casbin_rule.ptype.chars().next().is_some() {
            return self.normalize_policy(casbin_rule);
        }

        None
    }

    fn normalize_policy(&self, casbin_rule: &CasbinRule) -> Option<Vec<String>> {
        let mut result = vec![
            &casbin_rule.v0,
            &casbin_rule.v1,
            &casbin_rule.v2,
            &casbin_rule.v3,
            &casbin_rule.v4,
            &casbin_rule.v5,
        ];

        while let Some(last) = result.last() {
            if last.is_empty() {
                result.pop();
            } else {
                break;
            }
        }

        if !result.is_empty() {
            return Some(result.iter().map(|&x| x.to_owned()).collect());
        }

        None
    }
}

#[async_trait]
impl Adapter for SqlxAdapter {
    async fn load_policy(&mut self, m: &mut dyn Model) -> Result<()> {
        let rules = adapter::load_policy(&self.pool).await?;

        for casbin_rule in &rules {
            let rule = self.load_policy_line(casbin_rule);

            if let Some(ref sec) = casbin_rule.ptype.chars().next().map(|x| x.to_string()) {
                if let Some(t1) = m.get_mut_model().get_mut(sec) {
                    if let Some(t2) = t1.get_mut(&casbin_rule.ptype) {
                        if let Some(rule) = rule {
                            t2.get_mut_policy().insert(rule);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn load_filtered_policy<'a>(&mut self, m: &mut dyn Model, f: Filter<'a>) -> Result<()> {
        let rules = adapter::load_filtered_policy(&self.pool, &f).await?;
        self.is_filtered.store(true, Ordering::SeqCst);

        for casbin_rule in &rules {
            if let Some(policy) = self.normalize_policy(casbin_rule) {
                if let Some(ref sec) = casbin_rule.ptype.chars().next().map(|x| x.to_string()) {
                    if let Some(t1) = m.get_mut_model().get_mut(sec) {
                        if let Some(t2) = t1.get_mut(&casbin_rule.ptype) {
                            t2.get_mut_policy().insert(policy);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn save_policy(&mut self, m: &mut dyn Model) -> Result<()> {
        let mut rules = vec![];

        if let Some(ast_map) = m.get_model().get("p") {
            for (ptype, ast) in ast_map {
                let new_rules = ast
                    .get_policy()
                    .into_iter()
                    .filter_map(|x| self.save_policy_line(ptype, x));

                rules.extend(new_rules);
            }
        }

        if let Some(ast_map) = m.get_model().get("g") {
            for (ptype, ast) in ast_map {
                let new_rules = ast
                    .get_policy()
                    .into_iter()
                    .filter_map(|x| self.save_policy_line(ptype, x));

                rules.extend(new_rules);
            }
        }
        adapter::save_policy(&self.pool, rules).await
    }

    async fn add_policy(&mut self, _sec: &str, ptype: &str, rule: Vec<String>) -> Result<bool> {
        if let Some(new_rule) = self.save_policy_line(ptype, rule.as_slice()) {
            return adapter::add_policy(&self.pool, new_rule).await;
        }

        Ok(false)
    }

    async fn add_policies(
        &mut self,
        _sec: &str,
        ptype: &str,
        rules: Vec<Vec<String>>,
    ) -> Result<bool> {
        let new_rules = rules
            .iter()
            .filter_map(|x| self.save_policy_line(ptype, x))
            .collect::<Vec<NewCasbinRule>>();

        adapter::add_policies(&self.pool, new_rules).await
    }

    async fn remove_policy(&mut self, _sec: &str, pt: &str, rule: Vec<String>) -> Result<bool> {
        adapter::remove_policy(&self.pool, pt, rule).await
    }

    async fn remove_policies(
        &mut self,
        _sec: &str,
        pt: &str,
        rules: Vec<Vec<String>>,
    ) -> Result<bool> {
        adapter::remove_policies(&self.pool, pt, rules).await
    }

    async fn remove_filtered_policy(
        &mut self,
        _sec: &str,
        pt: &str,
        field_index: usize,
        field_values: Vec<String>,
    ) -> Result<bool> {
        if field_index <= 5 && !field_values.is_empty() && field_values.len() + field_index <= 6 {
            adapter::remove_filtered_policy(&self.pool, pt, field_index, field_values).await
        } else {
            Ok(false)
        }
    }

    async fn clear_policy(&mut self) -> Result<()> {
        adapter::clear_policy(&self.pool).await
    }

    fn is_filtered(&self) -> bool {
        self.is_filtered.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_owned(v: Vec<&str>) -> Vec<String> {
        v.into_iter().map(|x| x.to_owned()).collect()
    }

    #[cfg_attr(
        any(
            feature = "runtime-async-std-native-tls",
            feature = "runtime-async-std-rustls"
        ),
        async_std::test
    )]
    #[cfg_attr(
        any(feature = "runtime-tokio-native-tls", feature = "runtime-tokio-rustls"),
        tokio::test(flavor = "multi_thread")
    )]
    async fn test_create() {
        use casbin::prelude::*;

        let m = DefaultModel::from_file("examples/rbac_model.conf")
            .await
            .unwrap();

        let adapter = {
            #[cfg(feature = "postgres")]
            {
                SqlxAdapter::new("postgres://casbin_rs:casbin_rs@localhost:5432/casbin", 8)
                    .await
                    .unwrap()
            }

            #[cfg(feature = "mysql")]
            {
                SqlxAdapter::new("mysql://casbin_rs:casbin_rs@localhost:3306/casbin", 8)
                    .await
                    .unwrap()
            }

            #[cfg(feature = "sqlite")]
            {
                SqlxAdapter::new("sqlite:casbin.db", 8).await.unwrap()
            }
        };

        assert!(Enforcer::new(m, adapter).await.is_ok());
    }

    #[cfg_attr(
        any(
            feature = "runtime-async-std-native-tls",
            feature = "runtime-async-std-rustls"
        ),
        async_std::test
    )]
    #[cfg_attr(
        any(feature = "runtime-tokio-native-tls", feature = "runtime-tokio-rustls"),
        tokio::test(flavor = "multi_thread")
    )]
    async fn test_create_with_pool() {
        use casbin::prelude::*;

        let m = DefaultModel::from_file("examples/rbac_model.conf")
            .await
            .unwrap();
        let pool = {
            #[cfg(feature = "postgres")]
            {
                PgPoolOptions::new()
                    .max_connections(8)
                    .connect("postgres://casbin_rs:casbin_rs@localhost:5432/casbin")
                    .await
                    .unwrap()
            }

            #[cfg(feature = "mysql")]
            {
                MySqlPoolOptions::new()
                    .max_connections(8)
                    .connect("mysql://casbin_rs:casbin_rs@localhost:3306/casbin")
                    .await
                    .unwrap()
            }

            #[cfg(feature = "sqlite")]
            {
                SqlitePoolOptions::new()
                    .max_connections(8)
                    .connect("sqlite:casbin.db")
                    .await
                    .unwrap()
            }
        };

        let adapter = SqlxAdapter::new_with_pool(pool).await.unwrap();

        assert!(Enforcer::new(m, adapter).await.is_ok());
    }

    #[cfg_attr(
        any(
            feature = "runtime-async-std-native-tls",
            feature = "runtime-async-std-rustls"
        ),
        async_std::test
    )]
    #[cfg_attr(
        any(feature = "runtime-tokio-native-tls", feature = "runtime-tokio-rustls"),
        tokio::test(flavor = "multi_thread")
    )]
    async fn test_adapter() {
        use casbin::prelude::*;

        let file_adapter = FileAdapter::new("examples/rbac_policy.csv");

        let m = DefaultModel::from_file("examples/rbac_model.conf")
            .await
            .unwrap();

        let mut e = Enforcer::new(m, file_adapter).await.unwrap();
        let mut adapter = {
            #[cfg(feature = "postgres")]
            {
                SqlxAdapter::new("postgres://casbin_rs:casbin_rs@localhost:5432/casbin", 8)
                    .await
                    .unwrap()
            }

            #[cfg(feature = "mysql")]
            {
                SqlxAdapter::new("mysql://casbin_rs:casbin_rs@localhost:3306/casbin", 8)
                    .await
                    .unwrap()
            }

            #[cfg(feature = "sqlite")]
            {
                SqlxAdapter::new("sqlite:casbin.db", 8).await.unwrap()
            }
        };

        assert!(adapter.save_policy(e.get_mut_model()).await.is_ok());

        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .unwrap());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());

        assert!(adapter
            .remove_policies(
                "",
                "p",
                vec![
                    to_owned(vec!["alice", "data1", "read"]),
                    to_owned(vec!["bob", "data2", "write"]),
                    to_owned(vec!["data2_admin", "data2", "read"]),
                    to_owned(vec!["data2_admin", "data2", "write"]),
                ]
            )
            .await
            .is_ok());

        assert!(adapter
            .add_policies(
                "",
                "p",
                vec![
                    to_owned(vec!["alice", "data1", "read"]),
                    to_owned(vec!["bob", "data2", "write"]),
                    to_owned(vec!["data2_admin", "data2", "read"]),
                    to_owned(vec!["data2_admin", "data2", "write"]),
                ]
            )
            .await
            .is_ok());

        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["alice", "data1", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["bob", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "read"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "p", to_owned(vec!["data2_admin", "data2", "write"]))
            .await
            .is_ok());
        assert!(adapter
            .remove_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());

        assert!(!adapter
            .remove_policy(
                "",
                "g",
                to_owned(vec!["alice", "data2_admin", "not_exists"])
            )
            .await
            .unwrap());

        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "g", to_owned(vec!["alice", "data2_admin"]))
            .await
            .is_err());

        assert!(!adapter
            .remove_filtered_policy(
                "",
                "g",
                0,
                to_owned(vec!["alice", "data2_admin", "not_exists"]),
            )
            .await
            .unwrap());

        assert!(adapter
            .remove_filtered_policy("", "g", 0, to_owned(vec!["alice", "data2_admin"]))
            .await
            .unwrap());

        assert!(adapter
            .add_policy(
                "",
                "g",
                to_owned(vec!["alice", "data2_admin", "domain1", "domain2"]),
            )
            .await
            .is_ok());
        assert!(adapter
            .remove_filtered_policy(
                "",
                "g",
                1,
                to_owned(vec!["data2_admin", "domain1", "domain2"]),
            )
            .await
            .unwrap());

        // GitHub issue: https://github.com/apache/casbin-sqlx-adapter/issues/64
        assert!(adapter
            .add_policy("", "g", to_owned(vec!["carol", "data1_admin"]),)
            .await
            .is_ok());
        assert!(adapter
            .remove_filtered_policy("", "g", 0, to_owned(vec!["carol"]),)
            .await
            .unwrap());
        assert_eq!(vec![String::new(); 0], e.get_roles_for_user("carol", None));

        // GitHub issue: https://github.com/apache/casbin-sqlx-adapter/pull/90
        // add policies:
        // p, alice_rfp, book_rfp, read_rfp
        // p, bob_rfp, book_rfp, read_rfp
        // p, bob_rfp, book_rfp, write_rfp
        // p, alice_rfp, pen_rfp, get_rfp
        // p, bob_rfp, pen_rfp, get_rfp
        // p, alice_rfp, pencil_rfp, get_rfp
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice_rfp", "book_rfp", "read_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "book_rfp", "read_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "book_rfp", "write_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["alice_rfp", "pen_rfp", "get_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy("", "p", to_owned(vec!["bob_rfp", "pen_rfp", "get_rfp"]),)
            .await
            .is_ok());
        assert!(adapter
            .add_policy(
                "",
                "p",
                to_owned(vec!["alice_rfp", "pencil_rfp", "get_rfp"]),
            )
            .await
            .is_ok());

        // should remove (return true) all policies where "book_rfp" is in the second position
        assert!(adapter
            .remove_filtered_policy("", "p", 1, to_owned(vec!["book_rfp"]),)
            .await
            .unwrap());

        // should remove (return true) all policies which match "alice_rfp" on first position
        // and "get_rfp" on third position
        assert!(adapter
            .remove_filtered_policy("", "p", 0, to_owned(vec!["alice_rfp", "", "get_rfp"]),)
            .await
            .unwrap());

        // shadow the previous enforcer
        let mut e = Enforcer::new(
            "examples/rbac_with_domains_model.conf",
            "examples/rbac_with_domains_policy.csv",
        )
        .await
        .unwrap();

        assert!(adapter.save_policy(e.get_mut_model()).await.is_ok());
        e.set_adapter(adapter).await.unwrap();

        let filter = Filter {
            p: vec!["", "domain1"],
            g: vec!["", "", "domain1"],
        };

        e.load_filtered_policy(filter).await.unwrap();
        assert!(e.enforce(("alice", "domain1", "data1", "read")).unwrap());
        assert!(e.enforce(("alice", "domain1", "data1", "write")).unwrap());
        assert!(!e.enforce(("alice", "domain1", "data2", "read")).unwrap());
        assert!(!e.enforce(("alice", "domain1", "data2", "write")).unwrap());
        assert!(!e.enforce(("bob", "domain2", "data2", "read")).unwrap());
        assert!(!e.enforce(("bob", "domain2", "data2", "write")).unwrap());
    }
}
