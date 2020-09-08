use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct RootConfig {
    pub servers: Vec<ServerConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub connect: ConnectConfig,
    pub queries: Vec<QueryConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConnectConfig {
    pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct QueryConfig {
    pub name: String,
    pub sql: String,
    pub frequency: String,
}
