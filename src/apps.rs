use mongodb::bson::oid::ObjectId;
use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::serialize_option_oid_as_string;
use crate::roles::RoleItems;

pub const LOCAL_APP: &str = "local";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct App {
    #[serde(
        rename(serialize = "id", deserialize = "_id"),
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_oid_as_string"
    )]
    #[schema(value_type = String)]
    pub id: Option<ObjectId>,
    pub name: String,
    pub version: u64,
    pub default_role: RoleItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            id: None,
            name: LOCAL_APP.to_string(),
            version: 1,
            default_role: RoleItems::local(),
            created_at: None,
            updated_at: None,
        }
    }
}

impl App {
    pub fn id(&self) -> ObjectId {
        self.id.unwrap_or_default()
    }
}

#[derive(Serialize, ToSchema)]
pub struct AppsVec(pub Vec<App>);