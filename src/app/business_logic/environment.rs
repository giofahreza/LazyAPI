use std::sync::Arc;
use rayon::prelude::*;
use anyhow::anyhow;
use chrono::Utc;
use indexmap::map::MutableKeys;
use parking_lot::RwLock;
use thiserror::Error;
use tracing::trace;
use uuid::Uuid;

use crate::app::app::App;
use crate::app::business_logic::environment::EnvironmentError::{EnvironmentNotFound, KeyAlreadyExists, KeyNotFound};
use crate::app::files::environment::{OS_ENV_VARS, try_save_environment_to_file};
use crate::cli::args::ARGS;
use anyhow::{bail, Context};
use std::fs::{self, OpenOptions};
use crate::models::environment::Environment;

#[derive(Error, Debug)]
pub enum EnvironmentError {
    #[error("Environment not found")]
    EnvironmentNotFound,

    #[error("Key not found")]
    KeyNotFound,

    #[error("Key already exists")]
    KeyAlreadyExists,
}

impl App<'_> {
    pub fn get_selected_env_as_local(&self) -> Option<Arc<RwLock<Environment>>> {
        match self.environments.get(self.selected_environment) {
            None => None,
            Some(env) => Some(env.clone())
        }
    }

    pub fn get_env_as_local_from_index(&self, index: usize) -> Option<Arc<RwLock<Environment>>> {
        match self.environments.get(index) {
            None => None,
            Some(env) => Some(env.clone())
        }
    }

    pub fn find_environment(&self, environment_name: &str) -> anyhow::Result<usize> {
        trace!("Trying to find environment \"{environment_name}\"");

        let result = self.environments.par_iter().position_first(|environment| environment.read().name == environment_name );

        match result {
            None => {
                trace!("Not found");
                Err(anyhow!(EnvironmentNotFound))
            }
            Some(index) => {
                trace!("Found");
                Ok(index)
            }
        }
    }

    pub fn create_environment(&mut self, name: &str) -> anyhow::Result<usize> {
        let path = self.new_environment_path(name, None)?;
        if ARGS.should_save {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)] {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&path).context("Could not create environment file")?;
        }
        let index = self.environments.len();
        self.environments.push(Arc::new(RwLock::new(Environment {
            name: name.into(), path, ..Environment::default()
        })));
        Ok(index)
    }

    fn new_environment_path(&self, name: &str, except: Option<usize>) -> anyhow::Result<std::path::PathBuf> {
        if name.is_empty() || name.trim() != name || matches!(name, "." | "..")
            || name.chars().any(|ch| ch.is_control() || "/\\".contains(ch)) {
            bail!("Use a non-empty environment name without slashes or leading/trailing spaces");
        }
        if self.environments.iter().enumerate().any(|(i, env)| Some(i) != except && env.read().name == name) {
            bail!("An environment with that name already exists");
        }
        let path = ARGS.directory.as_ref().unwrap().join(format!(".env.{name}"));
        let unchanged = except.and_then(|i| self.environments.get(i)).is_some_and(|env| env.read().path == path);
        if !unchanged && path.symlink_metadata().is_ok() {
            bail!("An environment file with that name already exists");
        }
        Ok(path)
    }

    pub fn rename_environment(&mut self, index: usize, name: &str) -> anyhow::Result<()> {
        let path = self.new_environment_path(name, Some(index))?;
        let local = self.get_env_as_local_from_index(index).ok_or(EnvironmentNotFound)?;
        let mut env = local.write();
        if env.path == path { return Ok(()); }
        if ARGS.should_save {
            // Both paths are in the same directory. Linking refuses an existing
            // destination, and preserves the source bytes, comments and permissions.
            fs::hard_link(&env.path, &path).context("Could not rename environment file")?;
            if let Err(error) = fs::remove_file(&env.path) {
                let _ = fs::remove_file(&path);
                return Err(error).context("Could not remove old environment file");
            }
        }
        env.name = name.into();
        env.path = path;
        Ok(())
    }

    pub fn delete_environment(&mut self, index: usize) -> anyhow::Result<()> {
        let local = self.get_env_as_local_from_index(index).ok_or(EnvironmentNotFound)?;
        let mut env = local.write();
        if ARGS.should_save {
            fs::remove_file(&env.path).context("Could not delete environment file")?;
        }
        env.deleted = true;
        self.environments.remove(index);
        if self.selected_environment > index { self.selected_environment -= 1; }
        self.selected_environment = self.selected_environment.min(self.environments.len().saturating_sub(1));
        Ok(())
    }

    fn update_environment(&mut self, index: usize, update: impl FnOnce(&mut Environment) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let local = self.get_env_as_local_from_index(index).ok_or(EnvironmentNotFound)?;
        let mut env = local.write();
        let mut candidate = env.clone();
        update(&mut candidate)?;
        try_save_environment_to_file(&candidate)?;
        *env = candidate;
        Ok(())
    }

    fn validate_env_key(key: &str) -> anyhow::Result<()> {
        if key.is_empty() || key.chars().enumerate().any(|(i, ch)|
            !(ch == '_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit()))) {
            bail!("Variable names must start with a letter or underscore and contain only letters, numbers, or underscores");
        }
        Ok(())
    }

    pub fn get_env_value(&mut self, env_index: usize, key: &str) -> anyhow::Result<()> {
        let local = self.get_env_as_local_from_index(env_index).ok_or(EnvironmentNotFound)?;
        let env = local.read();
        println!("{}", env.values.get(key).ok_or(KeyNotFound)?);
        Ok(())
    }

    pub fn set_env_value(&mut self, env_index: usize, key: &str, value: String) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            *env.values.get_mut(key).ok_or(KeyNotFound)? = value;
            Ok(())
        })
    }

    pub fn set_env_value_by_index(&mut self, env_index: usize, key_index: usize, value: String) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            *env.values.get_index_mut(key_index).ok_or(KeyNotFound)?.1 = value;
            Ok(())
        })
    }

    pub fn create_env_value(&mut self, env_index: usize, key: Option<String>, value: String) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            let key = key.unwrap_or_else(|| {
                let mut suffix = env.values.len();
                while env.values.contains_key(&format!("KEY_{suffix}")) { suffix += 1; }
                format!("KEY_{suffix}")
            });
            Self::validate_env_key(&key)?;
            if env.values.contains_key(&key) { return Err(anyhow!(KeyAlreadyExists)); }
            env.values.insert(key, value);
            Ok(())
        })
    }

    pub fn delete_env_key(&mut self, env_index: usize, key: &str) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            env.values.shift_remove(key).ok_or(KeyNotFound)?;
            Ok(())
        })
    }

    pub fn delete_env_index(&mut self, env_index: usize, index: usize) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            env.values.shift_remove_index(index).ok_or(KeyNotFound)?;
            Ok(())
        })
    }

    pub fn rename_env_key(&mut self, env_index: usize, key: &str, new_key: &str) -> anyhow::Result<()> {
        let local = self.get_env_as_local_from_index(env_index).ok_or(EnvironmentNotFound)?;
        let index = local.read().values.get_index_of(key).ok_or(KeyNotFound)?;
        self.rename_env_key_by_index(env_index, index, new_key.into())
    }

    pub fn rename_env_key_by_index(&mut self, env_index: usize, key_index: usize, new_key: String) -> anyhow::Result<()> {
        self.update_environment(env_index, |env| {
            if env.values.get_index(key_index).ok_or(KeyNotFound)?.0 == &new_key { return Ok(()); }
            Self::validate_env_key(&new_key)?;
            if env.values.contains_key(&new_key) { return Err(anyhow!(KeyAlreadyExists)); }
            *env.values.get_index_mut2(key_index).ok_or(KeyNotFound)?.0 = new_key;
            Ok(())
        })
    }

    pub fn replace_env_keys_by_value(&self, input: &String) -> String {
        if self.environments.is_empty() {
            return input.to_string();
        }

        let mut tmp_string = input.to_string();

        let local_env = self.get_selected_env_as_local();

        if let Some(local_env) = local_env {
            let env = local_env.read();
            let mut values = env.values.clone();
            values.extend(OS_ENV_VARS.clone());
            
            for (key, value) in &values {
                tmp_string = tmp_string.replace(&format!("{{{{{}}}}}", key), value);
            }
        }

        
        tmp_string = tmp_string
            .replace("{{NOW}}", &Utc::now().to_string())
            .replace("{{TIMESTAMP}}", &Utc::now().timestamp().to_string())
            .replace("{{UUIDv4}}", &Uuid::new_v4().to_string())
            .replace("{{UUIDv7}}", &Uuid::now_v7().to_string());
        
        return tmp_string;
    }
}