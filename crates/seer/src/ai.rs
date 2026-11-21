//! Optional name suggestions from an AI model, with the person's own API key.
//!
//! Three services are supported: Anthropic (its own Messages API), and
//! OpenAI and OpenRouter (both through the OpenAI chat completions format).
//! Only text is sent: the file name and a few facts read from the file. The
//! media itself never leaves the Mac. Calls block, so run them off the main thread.

use crate::platform;
use anyhow::{Context, Result, anyhow, bail};
use security_framework::passwords::{get_generic_password, set_generic_password};
use seer_core::naming::{self, Identity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "codes.imran.seer";

const SYSTEM: &str = "You identify films and TV episodes for a media library tool. \
You are given a file name as it came from a download or a rip, plus facts read from the file itself. \
Work out which film or episode it is and return its title as media databases list it, \
in Latin script, with its original release year. \
The name usually carries quality, codec, language and site tags; none of that belongs in the title. \
Use the duration and audio languages to tell apart works that share a title. \
Give 0 for the year if you cannot tell it, and 0 for season and episode if this is a film. \
Set recognised to false when the name does not identify a published film or series, \
such as a camera file or a home video. A wrong title is worse than none, so do not guess.";

/// For services that may not enforce the schema: say the shape in words too.
const REPLY_FORMAT: &str = " Reply with one JSON object and nothing else, with exactly these keys: \
recognised (boolean), title (string), year (integer), season (integer), episode (integer).";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Anthropic,
    OpenAi,
    OpenRouter,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Anthropic, Provider::OpenAi, Provider::OpenRouter];

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude",
            Provider::OpenAi => "OpenAI",
            Provider::OpenRouter => "OpenRouter",
        }
    }

    /// The model used when none is typed. Only Anthropic has one: model
    /// names at the other services change too often to hard-code.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-opus-5-5",
            Provider::OpenAi | Provider::OpenRouter => "",
        }
    }

    pub fn model_hint(self) -> &'static str {
        match self {
            Provider::Anthropic => "Model, e.g. claude-opus-5-5",
            Provider::OpenAi => "Model name from platform.openai.com",
            Provider::OpenRouter => "Model, as vendor/name from openrouter.ai",
        }
    }

    fn keychain_account(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic-api-key",
            Provider::OpenAi => "openai-api-key",
            Provider::OpenRouter => "openrouter-api-key",
        }
    }

    fn environment_variable(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAi => "OPENAI_API_KEY",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    /// The stored key: from the Keychain, else the service's usual variable.
    pub fn api_key(self) -> Option<String> {
        get_generic_password(KEYCHAIN_SERVICE, self.keychain_account())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .or_else(|| std::env::var(self.environment_variable()).ok())
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
    }

    /// Stores the key in the login Keychain, replacing any earlier one.
    pub fn save_api_key(self, key: &str) -> Result<()> {
        set_generic_password(
            KEYCHAIN_SERVICE,
            self.keychain_account(),
            key.trim().as_bytes(),
        )
        .with_context(|| format!("could not save the key to the {}", platform::key_store()))
    }
}

/// Which service and model to ask. Keys are not here; they live in the Keychain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: Provider,
    anthropic_model: String,
    openai_model: String,
    openrouter_model: String,
}

impl Settings {
    fn file() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("Seer").join("ai.json"))
    }

    pub fn load() -> Settings {
        Self::file()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(file) = Self::file() else { return };
        let write = || -> Result<()> {
            std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
            std::fs::write(&file, serde_json::to_vec_pretty(self)?)?;
            Ok(())
        };
        if let Err(e) = write() {
            log::warn!("could not save AI settings: {e:#}");
        }
    }

    /// The model for a service: what was typed, else its default. May be empty.
    pub fn model(&self, provider: Provider) -> String {
        let typed = match provider {
            Provider::Anthropic => &self.anthropic_model,
            Provider::OpenAi => &self.openai_model,
            Provider::OpenRouter => &self.openrouter_model,
        };
        match typed.trim() {
            "" => provider.default_model().to_owned(),
            typed => typed.to_owned(),
        }
    }

    pub fn set_model(&mut self, provider: Provider, model: &str) {
        let slot = match provider {
            Provider::Anthropic => &mut self.anthropic_model,
            Provider::OpenAi => &mut self.openai_model,
            Provider::OpenRouter => &mut self.openrouter_model,
        };
        *slot = model.trim().to_owned();
    }
}

/// What is sent about a file.
pub struct Query {
    pub file_name: String,
    /// The container's own title tag, when it has one.
    pub title_tag: Option<String>,
    pub duration: Option<String>,
    pub audio_languages: Vec<String>,
}

/// Asks the chosen service which film or episode this is and returns a
/// library name for it.
pub fn suggest_name(provider: Provider, model: &str, key: &str, query: &Query) -> Result<String> {
    let mut facts = format!("File name: {}", query.file_name);
    if let Some(title) = &query.title_tag {
        facts.push_str(&format!("\nTitle tag inside the file: {title}"));
    }
    if let Some(duration) = &query.duration {
        facts.push_str(&format!("\nDuration: {duration}"));
    }
    if !query.audio_languages.is_empty() {
        facts.push_str(&format!(
            "\nAudio languages: {}",
            query.audio_languages.join(", ")
        ));
    }

    let answer = match provider {
        Provider::Anthropic => ask_anthropic(model, key, &facts)?,
        Provider::OpenAi => {
            ask_openai_compatible("https://api.openai.com/v1", provider, model, key, &facts)?
        }
        Provider::OpenRouter => {
            ask_openai_compatible("https://openrouter.ai/api/v1", provider, model, key, &facts)?
        }
    };

    let title = answer["title"].as_str().unwrap_or("").trim();
    if answer["recognised"] != true || title.is_empty() {
        bail!(
            "{} could not tell which film or series this is from its name.",
            provider.label()
        );
    }
    let number = |field: &str| answer[field].as_u64().unwrap_or(0);
    let (stem, extension) = match query.file_name.rsplit_once('.') {
        Some((stem, ext)) => (stem, Some(ext)),
        None => (query.file_name.as_str(), None),
    };
    let identity = Identity {
        title: title.to_owned(),
        year: u16::try_from(number("year"))
            .ok()
            .filter(|y| (1880..=2100).contains(y)),
        // An id is only ever copied from the file name. A model would make one up.
        imdb: naming::parse(stem).and_then(|parsed| parsed.imdb),
        episode: (number("season") > 0 && number("episode") > 0)
            .then(|| (number("season") as u32, number("episode") as u32)),
    };
    Ok(identity.file_name(extension))
}

fn answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "recognised": { "type": "boolean" },
            "title": { "type": "string" },
            "year": { "type": "integer" },
            "season": { "type": "integer" },
            "episode": { "type": "integer" }
        },
        "required": ["recognised", "title", "year", "season", "episode"],
        "additionalProperties": false
    })
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(90)))
        // Read error bodies ourselves; they say what went wrong.
        .http_status_as_error(false)
        .build()
        .into()
}

/// Turns a failed HTTP status into words a person can act on.
fn explain(provider: Provider, status: u16, reply: &Value) -> anyhow::Error {
    let detail = reply["error"]["message"]
        .as_str()
        .unwrap_or("no details given");
    let service = provider.label();
    anyhow!(match status {
        401 => format!("{service} rejected the API key. Check it and try again."),
        403 => format!("This key is not allowed to do that: {detail}"),
        404 => format!("{service} does not know that model: {detail}"),
        429 =>
            format!("{service} says the rate or spending limit is reached. Try again in a moment."),
        500.. => format!("{service} had a problem ({status}). Try again shortly."),
        _ => format!("{service} refused the request ({status}): {detail}"),
    })
}

/// Anthropic's Messages API, with a schema-constrained reply.
fn ask_anthropic(model: &str, key: &str, facts: &str) -> Result<Value> {
    let body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": SYSTEM,
        "messages": [{ "role": "user", "content": facts }],
        // If the model declines, let the API retry on its default fallback.
        "fallbacks": "default",
        "output_config": {
            // A lookup, not a reasoning task.
            "effort": "low",
            "format": { "type": "json_schema", "schema": answer_schema() }
        }
    });
    let mut response = agent()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2026-10-21")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .send_json(&body)
        .map_err(|e| anyhow!("Could not reach Anthropic: {e}"))?;
    let status = response.status().as_u16();
    let reply: Value = response
        .body_mut()
        .read_json()
        .context("The reply was not valid JSON")?;
    if status != 200 {
        return Err(explain(Provider::Anthropic, status, &reply));
    }
    match reply["stop_reason"].as_str() {
        Some("refusal") => bail!("Claude declined to answer for this file."),
        Some("max_tokens") => bail!("The answer was cut off. Try again."),
        _ => {}
    }
    let text = reply["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|block| block["text"].as_str())
        .context("The reply had no answer in it")?;
    parse_answer(text)
}

/// The OpenAI chat completions format, which OpenRouter also speaks.
fn ask_openai_compatible(
    base: &str,
    provider: Provider,
    model: &str,
    key: &str,
    facts: &str,
) -> Result<Value> {
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": format!("{SYSTEM}{REPLY_FORMAT}") },
            { "role": "user", "content": facts }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "identity", "strict": true, "schema": answer_schema() }
        }
    });
    let mut response = agent()
        .post(&format!("{base}/chat/completions"))
        .header("Authorization", &format!("Bearer {key}"))
        .send_json(&body)
        .map_err(|e| anyhow!("Could not reach {}: {e}", provider.label()))?;
    let status = response.status().as_u16();
    let reply: Value = response
        .body_mut()
        .read_json()
        .context("The reply was not valid JSON")?;
    if status != 200 {
        return Err(explain(provider, status, &reply));
    }
    // OpenRouter can report an upstream failure inside a 200.
    if let Some(message) = reply["error"]["message"].as_str() {
        bail!("{} reported a problem: {message}", provider.label());
    }
    let choice = &reply["choices"][0];
    if let Some(refusal) = choice["message"]["refusal"]
        .as_str()
        .filter(|r| !r.is_empty())
    {
        bail!("The model declined to answer: {refusal}");
    }
    if choice["finish_reason"] == "length" {
        bail!("The answer was cut off. Try again.");
    }
    let text = choice["message"]["content"]
        .as_str()
        .context("The reply had no answer in it")?;
    parse_answer(text)
}

/// Reads the answer object. Models that ignore the schema often wrap the
/// JSON in prose or a code fence, so fall back to the outermost braces.
fn parse_answer(text: &str) -> Result<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(text.trim()) {
        return Ok(value);
    }
    let (start, end) = (text.find('{'), text.rfind('}'));
    match (start, end) {
        (Some(start), Some(end)) if start < end => serde_json::from_str(&text[start..=end])
            .context("The model did not answer in the expected form. Try a different model."),
        _ => bail!("The model did not answer in the expected form. Try a different model."),
    }
}

#[cfg(test)]
mod tests {
    // Not a glob: the kit exports its own `test` attribute.
    use super::{Provider, Settings, parse_answer};

    #[test]
    fn reads_answers_with_and_without_wrapping() {
        let plain = r#"{"recognised":true,"title":"Leo","year":2023,"season":0,"episode":0}"#;
        assert_eq!(parse_answer(plain).unwrap()["title"], "Leo");
        let fenced = format!("Here you go:\n```json\n{plain}\n```");
        assert_eq!(parse_answer(&fenced).unwrap()["year"], 2023);
        assert!(parse_answer("I am not sure.").is_err());
    }

    #[test]
    fn models_fall_back_to_the_default_only_where_there_is_one() {
        let mut settings = Settings::default();
        assert_eq!(settings.model(Provider::Anthropic), "claude-opus-5-5");
        assert_eq!(settings.model(Provider::OpenRouter), "");
        settings.set_model(Provider::OpenRouter, "  vendor/name ");
        assert_eq!(settings.model(Provider::OpenRouter), "vendor/name");
        settings.set_model(Provider::Anthropic, "");
        assert_eq!(settings.model(Provider::Anthropic), "claude-opus-5-5");
    }
}
