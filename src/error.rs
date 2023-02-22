use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("MongoDB find API error. {0}")]
    MongoFind(mongodb::error::Error),
    #[error("MongoDB find one API error. {0}")]
    MongoFindOne(mongodb::error::Error),
    #[error("Could not parse MongoDB URI. {0}")]
    MongoParseUri(mongodb::error::Error),
    #[error("Could not create MongoDB client. {0}")]
    MongoCreateClient(mongodb::error::Error),
    #[error("Could not read MongoDB cursor. {0}")]
    MongoReadCursor(mongodb::error::Error),
    #[error("Could not watch a MongoDB watch stream. {0}")]
    MongoWatchChangeStream(mongodb::error::Error),
    #[error("Missing APP in database.")]
    MissingAppInDatabase,
    #[error("Invalid Unicode string.")]
    InvalidUnicodeString,
    #[error("Invalid authorization path: {0}")]
    InvalidAuthPath(String),
    #[error("Missing value.")]
    MissingValue,
    #[error("Missing parent path.")]
    MissingParentPath,
    #[error("Missing value name.")]
    MissingValueName,
    #[error("Missing value extension.")]
    MissingValueExtension,
    #[error("Missing last item.")]
    MissingLastItem,
    #[error("Invalid data value type.")]
    InvalidDataValueType,
}
