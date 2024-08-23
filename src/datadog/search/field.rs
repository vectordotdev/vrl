use super::grammar;

/// Default fields that represent the search path when a Datadog tag/facet is not provided.
static DEFAULT_FIELDS: &[&str] = &[
    "message",
    "custom.error.message",
    "custom.error.stack",
    "custom.title",
    "_default_",
];

/// Attributes that represent special fields in Datadog.
static RESERVED_ATTRIBUTES: &[&str] = &[
    "host",
    "source",
    "status",
    "service",
    "trace_id",
    "message",
    "timestamp",
    "tags",
];

/// Describes a field to search on.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum Field {
    /// Default field (when tag/facet isn't provided)
    Default(String),

    // TODO investigate making this be an enum which may make more sense
    //      when dealing with a fixed set of field names
    /// Reserved field that receives special treatment in Datadog.
    Reserved(String),

    /// An Attribute-- i.e. started with `@`.
    // In Datadog Log Search the `@` prefix is used to define a Facet for
    // attribute searching, and the event structure is assumed to have a
    // root level field "custom". In VRL we do not guarantee this event
    // structure so we are diverging a little from the DD Log Search
    // definition and implementation a bit here, by calling this "Attribute".
    //
    // Internally when we handle this enum variant, we attempt to parse the
    // string as a log path to obtain the value.
    Attribute(String),

    /// Tag type - i.e. search in the `tags` field.
    Tag(String),
}

impl Field {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Default(ref s) => s,
            Self::Reserved(ref s) => s,
            Self::Attribute(ref s) => s,
            Self::Tag(ref s) => s,
        }
    }
}

/// Converts a field/facet name to the VRL equivalent. Datadog payloads have a `message` field
/// (which is used whenever the default field is encountered.
pub fn normalize_fields<T: AsRef<str>>(value: T) -> Vec<Field> {
    let value = value.as_ref();
    if value.eq(grammar::DEFAULT_FIELD) {
        return DEFAULT_FIELDS
            .iter()
            .map(|s| Field::Default((*s).to_owned()))
            .collect();
    }

    let field = match value.replace('@', ".") {
        v if value.starts_with('@') => Field::Attribute(v),
        v if DEFAULT_FIELDS.contains(&v.as_ref()) => Field::Default(v),
        v if RESERVED_ATTRIBUTES.contains(&v.as_ref()) => Field::Reserved(v),
        v => Field::Tag(v),
    };

    vec![field]
}
