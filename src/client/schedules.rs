//! API for endpoints under `api/client/servers/{server}/schedules`

use crate::client::{PowerSignal, Server};
use crate::http::EmptyBody;
use crate::structs::{PteroList, PteroObject};
use reqwest::Method;
use serde::de::value::StringDeserializer;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::OffsetDateTime;

/// A task schedule for a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct Schedule {
    /// The ID of this schedule
    pub id: u64,

    /// The name of this schedule
    pub name: String,

    /// The rules for when this schedule is triggered
    pub cron: Cron,

    /// Whether this schedule is active
    pub is_active: bool,

    /// Whether this schedule is currently processing
    pub is_processing: bool,

    /// When this schedule was last run
    #[serde(deserialize_with = "crate::structs::optional_iso_time")]
    pub last_run_at: Option<OffsetDateTime>,

    /// When this schedule will next run
    #[serde(deserialize_with = "crate::structs::optional_iso_time")]
    pub next_run_at: Option<OffsetDateTime>,

    /// When this schedule was created
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,

    /// When this schedule was last updated
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub updated_at: OffsetDateTime,

    /// The tasks for this schedule
    pub relationships: ScheduleRelationships,
}

/// The rules for when a schedule is triggered
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Cron {
    /// The month(s) when a schedule is triggered
    #[serde(default = "cron_field_all")]
    pub month: CronField,

    /// The day(s) of the week when a schedule is triggered
    pub day_of_week: CronField,

    /// The day(s) of the month when a schedule is triggered
    pub day_of_month: CronField,

    /// The hour(s) of the day when the schedule is triggered
    pub hour: CronField,

    /// The minute(s) of the hour when the schedule is triggered
    pub minute: CronField,
}

fn cron_field_all() -> CronField {
    CronPart::all().into()
}

impl Default for Cron {
    fn default() -> Self {
        Cron {
            month: CronPart::all().into(),
            day_of_week: CronPart::all().into(),
            day_of_month: CronPart::all().into(),
            hour: 0.into(),
            minute: 0.into(),
        }
    }
}

/// Rules for a field of [`Cron`]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct CronField {
    #[allow(clippy::doc_markdown)]
    /// A set of rules for when a field triggers. These rules are ORed together
    pub parts: Vec<CronPart>,
}

/// A rule for when a field of [`Cron`] triggers
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum CronPart {
    /// Trigger on every minute/hour/etc
    All {
        /// Defaults to 1. If higher, trigger every `step` minutes/hours/etc
        step: u32,
    },
    /// Trigger on a specific minute/hour/etc
    Exact(u32),
    /// Trigger between a range of minutes/hours/etc
    Range {
        /// The minimum minute/hour/etc to trigger on
        min: u32,
        /// The maximum minute/hour/etc to trigger on
        max: u32,
        /// Defaults to 1. If higher trigger every `step` minutes/hours/etc within this range
        step: u32,
    },
}

impl CronPart {
    /// Creates a [`CronPart`] that triggers on every minute/hour/etc
    pub fn all() -> Self {
        CronPart::All { step: 1 }
    }
}

macro_rules! cron_from_number {
    ($($ty:ty),*) => {
        $(
        impl From<$ty> for CronPart {
            fn from(value: $ty) -> Self {
                CronPart::Exact(value as u32)
            }
        }

        impl From<$ty> for CronField {
            fn from(value: $ty) -> Self {
                CronPart::from(value).into()
            }
        }
        )*
    }
}

cron_from_number!(i8, u8, i16, u16, i32, u32);

impl From<CronPart> for CronField {
    fn from(value: CronPart) -> Self {
        CronField { parts: vec![value] }
    }
}

impl Default for CronField {
    fn default() -> Self {
        0.into()
    }
}

impl<'de> Deserialize<'de> for CronField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        fn parse_cron_int<E: serde::de::Error>(value: &str, full_value: &str) -> Result<u32, E> {
            Ok(match value.parse() {
                Ok(result) => result,
                #[allow(clippy::match_same_arms)]
                Err(_) => match &value.to_lowercase()[..] {
                    "jan" => 1,
                    "feb" => 2,
                    "mar" => 3,
                    "apr" => 4,
                    "may" => 5,
                    "jun" => 6,
                    "jul" => 7,
                    "aug" => 8,
                    "sep" => 9,
                    "oct" => 10,
                    "nov" => 11,
                    "dec" => 12,
                    "sun" => 0,
                    "mon" => 1,
                    "tue" => 2,
                    "wed" => 3,
                    "thu" => 4,
                    "fri" => 5,
                    "sat" => 6,
                    _ => {
                        return Err(E::custom(format!(
                            "Cannot interpret \"{full_value}\" as cronjob syntax"
                        )))
                    }
                },
            })
        }

        let string: String = Deserialize::deserialize(deserializer)?;
        let string = string.replace(|c: char| c.is_whitespace(), "");
        Ok(CronField {
            parts: string
                .split(',')
                .map(|mut part| {
                    let mut step = 1;
                    if let Some(slash_index) = part.find('/') {
                        step = part[slash_index + 1..].parse().map_err(|_err| {
                            <D::Error as serde::de::Error>::custom(format!(
                                "Cannot interpret \"{part}\" as cronjob syntax"
                            ))
                        })?;
                        part = &part[..slash_index];
                    }
                    Ok(if part == "*" {
                        CronPart::All { step }
                    } else if let Some(dash_index) = part.find('-') {
                        let (from, to) = part.split_at(dash_index);
                        let to = &to[1..];
                        CronPart::Range {
                            min: parse_cron_int(from, part)?,
                            max: parse_cron_int(to, part)?,
                            step,
                        }
                    } else {
                        CronPart::Exact(parse_cron_int(part, part)?)
                    })
                })
                .collect::<Result<Vec<CronPart>, D::Error>>()?,
        })
    }
}

impl Serialize for CronField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = self
            .parts
            .iter()
            .map(|part| match part {
                CronPart::All { step } => {
                    if *step == 1 {
                        "*".to_owned()
                    } else {
                        format!("*/{}", *step)
                    }
                }
                CronPart::Exact(value) => value.to_string(),
                CronPart::Range { min, max, step } => {
                    if *step == 1 {
                        format!("{}-{}", *min, *max)
                    } else {
                        format!("{}-{}/{}", *min, *max, *step)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        string.serialize(serializer)
    }
}

/// The tasks for this schedule
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ScheduleRelationships {
    /// The tasks for this schedule
    #[serde(deserialize_with = "crate::structs::ptero_list")]
    pub tasks: Vec<ScheduleTask>,
}

/// A task in a schedule
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ScheduleTask {
    /// The ID of the task
    pub id: u64,

    /// The sequence ID of the task
    pub sequence_id: u64,

    /// The action performed by this task
    #[serde(flatten)]
    pub action: ScheduleAction,

    /// The time offset in seconds from when this schedule triggers that the task should trigger
    pub time_offset: i32,

    /// Whether this task is currently queued
    pub is_queued: bool,

    /// When this task was created
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,

    /// When this task was last updated
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub updated_at: OffsetDateTime,
}

/// An action performed by a task
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[non_exhaustive]
pub enum ScheduleAction {
    /// Send a command to the server
    Command(String),
    /// Send a power signal to the server
    Power(PowerSignal),
    /// Create a backup
    Backup {
        /// The files to ignore while creating the backup
        ignored_files: Vec<String>,
    },
}

impl<'de> Deserialize<'de> for ScheduleAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Proxy {
            action: String,
            payload: String,
        }
        let proxy: Proxy = Deserialize::deserialize(deserializer)?;
        Ok(match &proxy.action[..] {
            "command" => ScheduleAction::Command(proxy.payload),
            "power" => ScheduleAction::Power(Deserialize::deserialize(StringDeserializer::new(
                proxy.payload,
            ))?),
            "backup" => ScheduleAction::Backup {
                ignored_files: proxy
                    .payload
                    .split('\n')
                    .filter(|str| !str.is_empty())
                    .map(|str| str.to_owned())
                    .collect(),
            },
            _ => {
                return Err(<D::Error as serde::de::Error>::unknown_variant(
                    &proxy.action,
                    &["command", "power", "backup"],
                ))
            }
        })
    }
}

impl Serialize for ScheduleAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Proxy {
            action: &'static str,
            payload: String,
        }
        let proxy = match self {
            ScheduleAction::Command(command) => Proxy {
                action: "command",
                payload: command.clone(),
            },
            ScheduleAction::Power(signal) => Proxy {
                action: "power",
                payload: signal.to_string(),
            },
            ScheduleAction::Backup { ignored_files } => Proxy {
                action: "backup",
                payload: ignored_files.join("\n"),
            },
        };
        proxy.serialize(serializer)
    }
}

/// The parameters to create a schedule
#[derive(Debug, Serialize, Clone)]
pub struct ScheduleParams {
    name: String,
    #[serde(skip_serializing_if = "core::ops::Not::not")]
    is_active: bool,
    #[serde(flatten)]
    cron: Cron,
}

impl ScheduleParams {
    /// Creates the default schedule parameters with the given name
    pub fn new(name: impl Into<String>) -> Self {
        ScheduleParams {
            name: name.into(),
            is_active: false,
            cron: Cron::default(),
        }
    }

    /// Sets the schedule to be active once created
    pub fn set_active(self) -> Self {
        ScheduleParams {
            is_active: true,
            ..self
        }
    }

    /// Sets the month(s) of the year the schedule will run at
    pub fn with_month(self, month: impl Into<CronField>) -> Self {
        ScheduleParams {
            cron: Cron {
                month: month.into(),
                ..self.cron
            },
            ..self
        }
    }

    /// Sets the minute(s) of the hour the schedule will run at
    pub fn with_minute(self, minute: impl Into<CronField>) -> Self {
        ScheduleParams {
            cron: Cron {
                minute: minute.into(),
                ..self.cron
            },
            ..self
        }
    }

    /// Sets the hour(s) of the day the schedule will run at
    pub fn with_hour(self, hour: impl Into<CronField>) -> Self {
        ScheduleParams {
            cron: Cron {
                hour: hour.into(),
                ..self.cron
            },
            ..self
        }
    }

    /// Sets the day(s) of the week the schedule will run at
    pub fn with_day_of_week(self, day_of_week: impl Into<CronField>) -> Self {
        ScheduleParams {
            cron: Cron {
                day_of_week: day_of_week.into(),
                ..self.cron
            },
            ..self
        }
    }

    /// Sets the day(s) of the month the schedule will run at
    pub fn with_day_of_month(self, day_of_month: impl Into<CronField>) -> Self {
        ScheduleParams {
            cron: Cron {
                day_of_month: day_of_month.into(),
                ..self.cron
            },
            ..self
        }
    }
}

impl From<Schedule> for ScheduleParams {
    fn from(value: Schedule) -> Self {
        ScheduleParams {
            name: value.name,
            is_active: value.is_active,
            cron: value.cron,
        }
    }
}

/// The parameters to create a task
#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
pub struct TaskParams {
    #[serde(flatten)]
    action: ScheduleAction,
    time_offset: i32,
}

impl TaskParams {
    /// Creates the default task parameters with the given action
    pub fn new(action: ScheduleAction) -> Self {
        TaskParams {
            action,
            time_offset: 0,
        }
    }

    /// Sets the time offset of this task
    pub fn with_time_offset(self, offset: i32) -> Self {
        TaskParams {
            time_offset: offset,
            ..self
        }
    }
}

impl From<ScheduleAction> for TaskParams {
    fn from(value: ScheduleAction) -> Self {
        TaskParams::new(value)
    }
}

impl From<ScheduleTask> for TaskParams {
    fn from(value: ScheduleTask) -> Self {
        TaskParams {
            action: value.action,
            time_offset: value.time_offset,
        }
    }
}

impl Server<'_> {
    /// Lists the schedules on this server
    pub async fn list_schedules(&self) -> crate::Result<Vec<Schedule>> {
        self.client
            .request::<PteroList<Schedule>>(Method::GET, &format!("servers/{}/schedules", self.id))
            .await
            .map(|schedules| schedules.data)
    }

    /// Creates a schedule with the given parameters on this server
    pub async fn create_schedule(
        &self,
        schedule: impl Into<ScheduleParams>,
    ) -> crate::Result<Schedule> {
        self.client
            .request_with_body::<PteroObject<Schedule>, _>(
                Method::POST,
                &format!("servers/{}/schedules", self.id),
                &schedule.into(),
            )
            .await
            .map(|schedule| schedule.attributes)
    }

    /// Gets the schedule with the given ID
    pub async fn get_schedule(&self, id: u64) -> crate::Result<Schedule> {
        self.client
            .request::<PteroObject<Schedule>>(
                Method::GET,
                &format!("servers/{}/schedules/{}", self.id, id),
            )
            .await
            .map(|schedule| schedule.attributes)
    }

    /// Updates the schedule with the given ID
    pub async fn update_schedule(
        &self,
        id: u64,
        schedule: impl Into<ScheduleParams>,
    ) -> crate::Result<Schedule> {
        self.client
            .request_with_body::<PteroObject<Schedule>, _>(
                Method::POST,
                &format!("servers/{}/schedules/{}", self.id, id),
                &schedule.into(),
            )
            .await
            .map(|schedule| schedule.attributes)
    }

    /// Deletes the schedule with the given ID
    pub async fn delete_schedule(&self, id: u64) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(
                Method::DELETE,
                &format!("servers/{}/schedules/{}", self.id, id),
            )
            .await?;
        Ok(())
    }

    /// Adds a task to a schedule
    pub async fn create_task(
        &self,
        schedule_id: u64,
        task: impl Into<TaskParams>,
    ) -> crate::Result<ScheduleTask> {
        self.client
            .request_with_body::<PteroObject<ScheduleTask>, _>(
                Method::POST,
                &format!("servers/{}/schedules/{}/tasks", self.id, schedule_id),
                &task.into(),
            )
            .await
            .map(|task| task.attributes)
    }

    /// Updates a task in a schedule
    pub async fn update_task(
        &self,
        schedule_id: u64,
        task_id: u64,
        task: impl Into<TaskParams>,
    ) -> crate::Result<ScheduleTask> {
        self.client
            .request_with_body::<PteroObject<ScheduleTask>, _>(
                Method::POST,
                &format!(
                    "servers/{}/schedules/{}/tasks/{}",
                    self.id, schedule_id, task_id
                ),
                &task.into(),
            )
            .await
            .map(|task| task.attributes)
    }

    /// Deletes a task from a schedule
    pub async fn delete_task(&self, schedule_id: u64, task_id: u64) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(
                Method::DELETE,
                &format!(
                    "servers/{}/schedules/{}/tasks/{}",
                    self.id, schedule_id, task_id
                ),
            )
            .await?;
        Ok(())
    }
}
