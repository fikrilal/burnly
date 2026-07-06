use serde_json::Value;

use super::runtime_client::{RuntimeClient, RuntimeClientError};
use super::RuntimeEndpoint;

const TRAJECTORY_SUMMARIES_FIELD: &str = "trajectorySummaries";
const GENERATOR_METADATA_FIELD: &str = "generatorMetadata";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrajectorySummary {
    pub(crate) cascade_id: String,
    pub(crate) step_count: Option<u32>,
}

pub(crate) fn parse_trajectory_summaries(response: &Value) -> Vec<TrajectorySummary> {
    let Some(summaries) = response
        .get(TRAJECTORY_SUMMARIES_FIELD)
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    let mut parsed = Vec::with_capacity(summaries.len());
    for (key, summary) in summaries {
        let Some(cascade_id) = cascade_id_from_summary(key, summary) else {
            continue;
        };
        parsed.push(TrajectorySummary {
            cascade_id,
            step_count: step_count_from_summary(summary),
        });
    }
    parsed.sort_by(|left, right| left.cascade_id.cmp(&right.cascade_id));
    parsed
}

pub(crate) fn generator_metadata_items(response: &Value) -> Vec<Value> {
    response
        .get(GENERATOR_METADATA_FIELD)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn list_trajectory_summaries(
    client: &RuntimeClient,
    endpoint: &RuntimeEndpoint,
) -> Result<Vec<TrajectorySummary>, RuntimeClientError> {
    let response = client.get_all_cascade_trajectories(endpoint)?;
    Ok(parse_trajectory_summaries(&response))
}

pub(crate) fn fetch_generator_metadata_items(
    client: &RuntimeClient,
    endpoint: &RuntimeEndpoint,
    cascade_id: &str,
) -> Result<Vec<Value>, RuntimeClientError> {
    if cascade_id.trim().is_empty() {
        return Err(RuntimeClientError::InvalidCascadeId);
    }
    let response = client.get_cascade_trajectory_generator_metadata(endpoint, cascade_id)?;
    Ok(generator_metadata_items(&response))
}

fn cascade_id_from_summary(key: &str, summary: &Value) -> Option<String> {
    first_non_empty_string([
        summary.get("cascadeId"),
        summary.get("trajectoryId"),
        summary.get("id"),
        summary.get("sessionId"),
        Some(&Value::String(key.to_owned())),
    ])
}

fn step_count_from_summary(summary: &Value) -> Option<u32> {
    first_u32([
        summary.get("stepCount"),
        summary.get("numSteps"),
        summary.get("totalSteps"),
    ])
}

fn first_non_empty_string(values: [Option<&Value>; 5]) -> Option<String> {
    values.into_iter().flatten().find_map(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

fn first_u32(values: [Option<&Value>; 3]) -> Option<u32> {
    values.into_iter().flatten().find_map(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| {
                value
                    .as_i64()
                    .and_then(|number| u32::try_from(number).ok())
            })
            .or_else(|| value.as_str().and_then(|text| text.parse::<u32>().ok()))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/infrastructure/collectors/antigravity/fixtures")
            .join(name)
    }

    #[test]
    fn parses_trajectory_list_fixture() {
        let fixture = fs::read_to_string(fixture_path("trajectory_list.json")).expect("fixture");
        let response: Value = serde_json::from_str(&fixture).expect("json");

        let summaries = parse_trajectory_summaries(&response);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].cascade_id, "conversation-a");
        assert_eq!(summaries[0].step_count, Some(12));
        assert_eq!(summaries[1].cascade_id, "conversation-b");
        assert_eq!(summaries[1].step_count, Some(7));
    }

    #[test]
    fn parses_generator_metadata_fixture() {
        let fixture =
            fs::read_to_string(fixture_path("generator_metadata.json")).expect("fixture");
        let response: Value = serde_json::from_str(&fixture).expect("json");

        let items = generator_metadata_items(&response);

        assert_eq!(items.len(), 2);
        assert!(items[0].get("chatModel").is_some());
    }

    #[test]
    fn resolves_cascade_id_from_summary_fields() {
        let summaries = parse_trajectory_summaries(&json!({
            "trajectorySummaries": {
                "conversation-key": {
                    "trajectoryId": "trajectory-only",
                    "stepCount": 3
                }
            }
        }));

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].cascade_id, "trajectory-only");
        assert_eq!(summaries[0].step_count, Some(3));
    }
}