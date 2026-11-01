//! The decision model over OpenRouter's `/systemone`: typed questions in, calibrated answers
//! out. The key comes from the caller's `OPENROUTER_API_KEY`; the model is `DESK_DECISION_MODEL`
//! or [`DEFAULT_MODEL`]. A request that gets no answer in time is sent once more (a decision is
//! short and cheap); any other failure is the caller's to see.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::model::{Answer, DecideError, Decider, Decision, Question};

/// The decision model used when none is named: a catalog id of OpenRouter's.
pub const DEFAULT_MODEL: &str = "~typesafe/jev-latest";
const BASE_URL: &str = "https://openrouter.ai/api/v1";
/// A decision that hangs is given up on, and asked once more.
const TIMEOUT: Duration = Duration::from_secs(90);

pub struct OpenRouter {
    agent: ureq::Agent,
    key: String,
    model: String,
}

impl OpenRouter {
    pub fn new(key: &str, model: Option<&str>) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            // The OS's TLS, the one feature built in: ureq's default is Rustls, which is not.
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .build(),
            )
            .build();
        Self {
            agent: config.into(),
            key: key.to_string(),
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
        }
    }

    /// One POST; a timeout is the only failure asked again.
    fn post(&self, body: &Value) -> Result<Value, DecideError> {
        let url = format!("{BASE_URL}/systemone");
        let mut last = DecideError::Unavailable("no answer".into());
        for _ in 0..2 {
            let sent = self
                .agent
                .post(&url)
                .header("Authorization", &format!("Bearer {}", self.key))
                .header("X-Title", "llx-desk")
                .send_json(body);
            match sent {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    let value: Value = response
                        .body_mut()
                        .read_json()
                        .map_err(|e| DecideError::Invalid(format!("unreadable answer: {e}")))?;
                    return match status {
                        200..=299 => Ok(value),
                        401 | 403 => Err(DecideError::Unavailable(format!(
                            "OpenRouter refused the key ({status}): check OPENROUTER_API_KEY"
                        ))),
                        _ => Err(DecideError::Unavailable(format!(
                            "OpenRouter answered {status}: {}",
                            value
                                .pointer("/error/message")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                        ))),
                    };
                }
                Err(ureq::Error::Timeout(_)) => {
                    last = DecideError::Unavailable("OpenRouter did not answer in time".into());
                }
                Err(e) => return Err(DecideError::Unavailable(format!("OpenRouter: {e}"))),
            }
        }
        Err(last)
    }
}

impl Decider for OpenRouter {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let questions: Map<String, Value> = questions
            .iter()
            .map(|(key, question)| (key.clone(), wire_question(question)))
            .collect();
        let body = json!({"model": self.model, "state": state, "questions": questions});
        let started = Instant::now();
        let value = self.post(&body)?;
        let number = |path: &str| value.pointer(path).and_then(Value::as_u64).unwrap_or(0);
        let mut decision = Decision {
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.model)
                .to_string(),
            cost: value
                .pointer("/usage/cost")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            millis: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            input_tokens: number("/usage/input_tokens"),
            output_tokens: number("/usage/output_tokens"),
            ..Decision::default()
        };
        let answers = value.get("answers").and_then(Value::as_object);
        for (key, answer) in answers.into_iter().flatten() {
            if let Some(answer) = read_answer(answer) {
                decision.answers.insert(key.clone(), answer);
            }
        }
        Ok(decision)
    }
}

fn wire_question(question: &Question) -> Value {
    match question {
        Question::Choice {
            instructions,
            criteria,
        } => {
            json!({"type": "choice", "instructions": instructions, "criteria": criteria})
        }
        Question::Noul { instructions } => json!({"type": "noul", "instructions": instructions}),
    }
}

/// An answer as the `/systemone` wire carries it.
fn read_answer(answer: &Value) -> Option<Answer> {
    match answer.get("type")?.as_str()? {
        "choice" => Some(Answer::Choice {
            choice: answer.get("choice")?.as_str()?.to_string(),
            probabilities: (answer.get("probabilities")?.as_object()?.iter())
                .filter_map(|(key, value)| Some((key.clone(), value.as_f64()?)))
                .collect(),
            confidence: answer
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        }),
        "noul" => Some(Answer::Noul(answer.get("noul")?.as_f64()?)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "openrouter_tests.rs"]
mod tests;
