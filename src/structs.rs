use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

pub(crate) fn bool_true() -> bool {
    true
}

pub(crate) fn iso_time<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let string: String = Deserialize::deserialize(deserializer)?;
    OffsetDateTime::parse(&string, &Iso8601::DEFAULT)
        .map_err(|err| <D::Error as serde::de::Error>::custom(format!("{err}")))
}

pub(crate) fn optional_iso_time<'de, D>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt_string: Option<String> = Deserialize::deserialize(deserializer)?;
    opt_string
        .map(|string| OffsetDateTime::parse(&string, &Iso8601::DEFAULT))
        .transpose()
        .map_err(|err| <D::Error as serde::de::Error>::custom(format!("{err}")))
}

#[derive(Deserialize)]
pub(crate) struct PteroObject<T> {
    pub(crate) attributes: T,
}

#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct PteroList<T>
where
    T: DeserializeOwned,
{
    #[serde(deserialize_with = "ptero_list")]
    pub(crate) data: Vec<T>,
}

#[derive(Deserialize)]
pub(crate) struct PteroData<T> {
    pub(crate) data: T,
}

pub(crate) fn default_on_null<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let option: Option<T> = Deserialize::deserialize(deserializer)?;
    Ok(option.unwrap_or_default())
}

pub(crate) fn ptero_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    struct PteroList<T> {
        data: Vec<PteroObject<T>>,
    }
    let ptero_list: PteroList<T> = Deserialize::deserialize(deserializer)?;
    Ok(ptero_list
        .data
        .into_iter()
        .map(|obj| obj.attributes)
        .collect())
}
