//! The kinds that draw themselves (`crate::kind`) in the tool's schema: each one's name among
//! the kinds a tab is opened as, and its fields among the tool's.

use serde_json::{Map, Value, json};

/// The fields of a kind that draws itself, and its name among the kinds a tab is opened as. A
/// field another kind has too keeps its type and says both: what it is on each, and every value
/// it takes on either.
pub(super) fn fields(fields: &mut Map<String, Value>, about: &crate::kind::About) {
    if let Some(kinds) = fields
        .get_mut("kind")
        .and_then(|kind| kind.get_mut("enum"))
        .and_then(Value::as_array_mut)
    {
        kinds.push(json!(about.id));
    }
    for (name, schema) in about.fields {
        let Ok(mut own) = serde_json::from_str::<Value>(schema) else {
            log::warn!("the {}'s field {name} has no schema that reads", about.id);
            continue;
        };
        let Some(had) = fields.get_mut(*name) else {
            fields.insert((*name).to_string(), own);
            continue;
        };
        let said = [&*had, &own].map(|field| field["description"].as_str().unwrap_or_default());
        had["description"] = json!(said.join(" ").trim());
        let more = own.get_mut("enum").and_then(Value::as_array_mut);
        match (had.get_mut("enum").and_then(Value::as_array_mut), more) {
            (Some(values), Some(more)) => {
                more.retain(|value| !values.contains(value));
                values.append(more);
            }
            // One of them takes any value: so does the field.
            (Some(_), None) => {
                had.as_object_mut().map(|had| had.remove("enum"));
            }
            (None, _) => {}
        }
    }
}
