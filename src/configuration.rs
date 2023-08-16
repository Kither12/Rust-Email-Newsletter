use std::env;

use secrecy::{ExposeSecret, Secret};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: secrecy::Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        ))
    }
    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port
        ))
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("configuration");

    let enviroment: Enviroment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_owned())
        .try_into()
        .expect("Failed to parse APP_ENVIROMENT");

    config::Config::builder()
        .add_source(config::File::from(base_path.join("base")).required(true))
        .add_source(config::File::from(base_path.join(enviroment.as_str())).required(true))
        .build()
        .and_then(|x| x.try_deserialize())
}

pub enum Enviroment {
    Local,
    Production,
}
impl Enviroment {
    pub fn as_str(&self) -> &str {
        match self {
            Enviroment::Local => "local",
            Enviroment::Production => "production",
        }
    }
}
impl TryFrom<String> for Enviroment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!("Expect local or production found {}", other)),
        }
    }
}
