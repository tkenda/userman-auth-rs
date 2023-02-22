use std::path::Path;

use mongodb::bson::oid::ObjectId;
use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};

use crate::{AuthError, Result};

use crate::{serialize_oid_as_string, serialize_option_oid_as_string};

pub const LOCAL_ROLE: &str = "local-default";

fn crud_item<T: Into<String>>(name: T) -> Item {
    Item {
        name: name.into(),
        values: RoleValues(vec![
            Value {
                name: "create".to_string(),
                data: DataValue::Boolean(true),
                options: None,
            },
            Value {
                name: "read".to_string(),
                data: DataValue::Boolean(true),
                options: None,
            },
            Value {
                name: "update".to_string(),
                data: DataValue::Boolean(true),
                options: None,
            },
            Value {
                name: "delete".to_string(),
                data: DataValue::Boolean(true),
                options: None,
            },
        ]),
        items: RoleItems::default(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DataValue {
    String(String),
    Float(f64),
    Integer(i64),
    Boolean(bool),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataOptions {
    pub min_value: DataValue,
    pub max_value: DataValue,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    pub name: String,
    #[serde(flatten)]
    pub data: DataValue,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub options: Option<DataOptions>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoleValues(pub Vec<Value>);

impl RoleValues {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn inner(&self) -> &Vec<Value> {
        self.0.as_ref()
    }

    pub fn inner_mut(&mut self) -> &mut Vec<Value> {
        self.0.as_mut()
    }

    pub fn find(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|el| el.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.0.iter_mut().find(|el| el.name == name)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub name: String,
    #[serde(skip_serializing_if = "RoleValues::is_empty", default)]
    pub values: RoleValues,
    #[serde(skip_serializing_if = "RoleItems::is_empty", default)]
    pub items: RoleItems,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoleItems(Vec<Item>);

impl RoleItems {
    pub fn local() -> Self {
        Self(vec![
            crud_item("users"),
            crud_item("roles"),
            crud_item("apps"),
        ])
    }

    pub fn new(src: Vec<Item>) -> Self {
        Self(src)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn inner_mut(&mut self) -> &mut Vec<Item> {
        self.0.as_mut()
    }

    pub fn find(&self, name: &str) -> Option<&Item> {
        self.0.iter().find(|&el| el.name == name)
    }

    pub fn find_value<P: ?Sized + AsRef<Path>>(&self, src: &P) -> Result<DataValue> {
        let mut cursor = self;

        let path = src.as_ref();

        let items = match path.parent() {
            Some(t) => t,
            None => return Err(AuthError::MissingParentPath),
        };

        for part in items.iter().take(items.iter().count() - 2) {
            let name = part.to_str().ok_or(AuthError::InvalidUnicodeString)?;

            if name != "/" {
                cursor = match cursor.find(name) {
                    Some(t) => &t.items,
                    None => return Err(AuthError::InvalidAuthPath(name.to_string())),
                }
            }
        }

        let last_part = match items.iter().last() {
            Some(t) => t.to_owned(),
            None => return Err(AuthError::MissingLastItem),
        };

        let last_name = last_part.to_str().ok_or(AuthError::InvalidUnicodeString)?;

        let last_item = match cursor.find(last_name) {
            Some(t) => t,
            None => return Err(AuthError::InvalidAuthPath(last_name.to_string())),
        };

        let value_name_part = match path.file_stem() {
            Some(t) => t,
            None => return Err(AuthError::MissingValueName),
        };

        let value_name = value_name_part
            .to_str()
            .ok_or(AuthError::InvalidUnicodeString)?;

        let value_ext_part = match path.extension() {
            Some(t) => t,
            None => return Err(AuthError::MissingValueExtension),
        };

        let value_ext = value_ext_part
            .to_str()
            .ok_or(AuthError::InvalidUnicodeString)?;

        let value = last_item
            .values
            .find(value_name)
            .ok_or(AuthError::MissingValue)?;

        match (value_ext, &value.data) {
            ("boolean", DataValue::Boolean(_)) => {}
            ("float", DataValue::Float(_)) => {}
            ("integer", DataValue::Integer(_)) => {}
            ("string", DataValue::String(_)) => {}
            _ => return Err(AuthError::InvalidDataValueType),
        }

        Ok(value.data.clone())
    }

    fn merge_items(&self, new: &mut Vec<Item>) {
        for n_item in new {
            if let Some(a_item) = self.find(&n_item.name) {
                // values
                for n_value in n_item.values.inner_mut() {
                    if let Some(a_value) = a_item.values.find(&n_value.name) {
                        *n_value = a_value.clone();
                    }
                }

                // sub-items
                let n_sub_items = n_item.items.inner_mut();
                a_item.items.merge_items(n_sub_items);
            }
        }
    }

    /// Merge the new &mut RoleItems collection with self RoleItems.
    /// Look for new &mut RoleItems values in self collection. If the
    /// value exists, update it with self collection value.
    pub fn merge(&self, new: &mut RoleItems) {
        self.merge_items(new.inner_mut());
    }

    fn add_items(&self, new: &mut Vec<Item>) {
        for item in &self.0 {
            match new.iter_mut().find(|t| t.name == item.name) {
                Some(t) => {
                    for value in item.values.inner() {
                        match t.values.find_mut(&value.name) {
                            Some(t) => match value.data {
                                DataValue::Boolean(t) if !t => {}
                                _ => {
                                    t.data = value.data.clone();
                                }
                            },
                            None => {
                                t.values.inner_mut().push(value.clone());
                            }
                        }
                    }

                    item.items.add_items(t.items.inner_mut());
                }
                None => {
                    new.push(item.clone());
                }
            }
        }

        for n_item in new {
            if let Some(a_item) = self.find(&n_item.name) {
                // values
                for n_value in n_item.values.inner_mut() {
                    if let Some(a_value) = a_item.values.find(&n_value.name) {
                        *n_value = a_value.clone();
                    }
                }

                // sub-items
                let n_sub_items = n_item.items.inner_mut();
                a_item.items.merge_items(n_sub_items);
            }
        }
    }

    /// Add the new &mut RoleItems collection with self RoleItems.
    /// Look for new &mut RoleItems values in self collection.
    /// If the value is Boolean(true), set to true.
    /// If the value is Boolean(false), don't change.
    /// If the value is not a Boolean, replace with the last one.
    /// If the value is missing add the value.
    pub fn add(&self, new: &mut RoleItems) {
        self.add_items(new.inner_mut());
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    #[serde(
        rename(serialize = "id", deserialize = "_id"),
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_oid_as_string"
    )]
    pub id: Option<ObjectId>,
    #[serde(serialize_with = "serialize_oid_as_string")]
    pub app: ObjectId,
    pub name: String,
    pub items: RoleItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime>,
}

impl Default for Role {
    fn default() -> Self {
        Self {
            id: None,
            app: ObjectId::default(),
            name: LOCAL_ROLE.to_string(),
            items: RoleItems::default(),
            created_at: None,
            updated_at: None,
        }
    }
}

impl Role {
    pub fn id(&self) -> ObjectId {
        self.id.unwrap_or_default()
    }

    pub fn to_string_pretty(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self)
    }
}
