use futures::StreamExt;
use futures::TryStreamExt;
use haikunator::Haikunator;
use log::error;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use role::RoleItems;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod app;
mod error;
pub mod role;

use app::App;
use role::Role;

pub use error::AuthError;
pub type Result<T> = std::result::Result<T, AuthError>;

const APPS: &str = "apps";
const ROLES: &str = "roles";

fn serialize_oid_as_string<S>(oid: &ObjectId, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(oid.to_string().as_str())
}

fn serialize_option_oid_as_string<S>(
    oid: &Option<ObjectId>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match oid {
        Some(ref t) => serializer.serialize_some(t.to_string().as_str()),
        None => serializer.serialize_none(),
    }
}

#[derive(Clone, Debug)]
pub struct Roles(Arc<RwLock<HashMap<String, Role>>>);

impl Default for Roles {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

impl Roles {
    async fn set(&self, src: HashMap<String, Role>) {
        let mut lock = self.0.write().await;
        *lock = src;
    }

    async fn get<'r, T: Into<&'r str>>(&self, name: T) -> Option<Role> {
        let lock = self.0.read().await;
        lock.get(name.into()).cloned()
    }
}

#[derive(Clone, Debug)]
pub struct Auth {
    roles: Roles,
    database: Database,
    app_name: String,
}

impl Auth {
    pub async fn add_role_items(&self, role_names: Vec<String>) -> RoleItems {
        let mut parent = RoleItems::default();

        for name in role_names {
            if let Some(role) = self.roles.get(name.as_str()).await {
                role.items.add(&mut parent);
            }
        }

        parent
    }
}

#[derive(Debug)]
pub struct MongoDB {
    uri: String,
    db_name: String,
    client_name: String,
}

impl Default for MongoDB {
    fn default() -> Self {
        Self {
            uri: String::from("mongodb://localhost:27017"),
            db_name: String::from("umt"),
            client_name: Haikunator::default().haikunate(),
        }
    }
}

#[derive(Debug)]
pub struct AuthBuilder {
    mongodb: MongoDB,
    app_name: String,
}

impl AuthBuilder {
    pub fn mongodb_uri<T: Into<String>>(&mut self, src: T) -> &mut Self {
        self.mongodb.uri = src.into();
        self
    }

    pub fn mongodb_db_name<T: Into<String>>(&mut self, src: T) -> &mut Self {
        self.mongodb.db_name = src.into();
        self
    }

    pub fn mongodb_app_name<T: Into<String>>(&mut self, src: T) -> &mut Self {
        self.mongodb.client_name = src.into();
        self
    }

    pub async fn build(self) -> Result<Auth> {
        let mut client_options = ClientOptions::parse(&self.mongodb.uri)
            .await
            .map_err(AuthError::MongoParseUri)?;

        client_options.app_name = Some(self.mongodb.client_name.to_owned());

        let client = Client::with_options(client_options).map_err(AuthError::MongoCreateClient)?;

        let database = client.database(&self.mongodb.db_name);

        Ok(Auth {
            roles: Roles::default(),
            database,
            app_name: self.app_name,
        })
    }
}

impl Auth {
    pub fn builder<T: Into<String>>(app_name: T) -> AuthBuilder {
        AuthBuilder {
            mongodb: MongoDB::default(),
            app_name: app_name.into(),
        }
    }

    async fn update_roles(&self) -> Result<()> {
        // get app id
        let app = self
            .database
            .collection::<App>(APPS)
            .find_one(doc! { "name": &self.app_name }, None)
            .await
            .map_err(AuthError::MongoFindOne)?;

        match app {
            Some(t) => {
                let mut cursor = self
                    .database
                    .collection::<Role>(ROLES)
                    .find(
                        doc! {
                            "app": t.id()
                        },
                        None,
                    )
                    .await
                    .map_err(AuthError::MongoFind)?;

                let mut roles = HashMap::new();

                while let Some(role) = cursor
                    .try_next()
                    .await
                    .map_err(AuthError::MongoReadCursor)?
                {
                    roles.insert(role.name.clone(), role);
                }

                self.roles.set(roles).await;

                Ok(())
            }
            None => Err(AuthError::MissingAppInDatabase),
        }
    }

    pub async fn init(&self) -> Result<()> {
        self.update_roles().await?;

        let ref_self = self.clone();

        tokio::spawn(async move {
            let mut change_stream = match ref_self
                .database
                .collection::<Role>(ROLES)
                .watch(vec![], None)
                .await
                .map_err(AuthError::MongoWatchChangeStream)
            {
                Ok(t) => t,
                Err(err) => {
                    return error!("{}", err);
                }
            };

            while let Some(Ok(_)) = change_stream.next().await {
                if let Err(err) = ref_self.update_roles().await {
                    error!("{}", err);
                }
            }
        });

        Ok(())
    }
}
