use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::{postgres::PgConnectOptions, ConnectOptions};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: secrecy::Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

impl DatabaseSettings {
    pub fn with_db(&self) -> PgConnectOptions {
        let mut options = self.without_db().database(&self.database_name);
        options = options.log_statements(tracing::log::LevelFilter::Trace);
        options
    }
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            sqlx::postgres::PgSslMode::Require
        } else {
            sqlx::postgres::PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(&self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
    pub fn connection_string(&self) -> secrecy::Secret<String> {
        secrecy::Secret::new(format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        ))
    }
}

#[derive(serde::Deserialize)]
pub struct EmailClientSettings{
    pub user_name: String,
    pub user_mail: String,
    pub password: Secret<String>,
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
        .add_source(config::File::from(base_path.join("base")))
        .add_source(config::File::from(base_path.join(enviroment.as_str())))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
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
