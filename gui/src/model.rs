use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListPayload {
    #[serde(rename = "activeAccountNumber")]
    pub active_account_number: Option<u32>,
    #[serde(default)]
    pub accounts: Vec<AccountRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountRow {
    pub number: u32,
    pub email: String,
    #[serde(rename = "organizationName", default)]
    pub organization_name: String,
    #[serde(default)]
    pub active: bool,
    #[serde(rename = "usageStatus", default)]
    pub usage_status: String,
    pub usage: Option<Usage>,
    #[serde(rename = "lastGoodUsage")]
    pub last_good_usage: Option<Usage>,
    #[serde(rename = "lastGoodAgeSeconds")]
    pub last_good_age_seconds: Option<f64>,
    #[serde(rename = "usageAgeSeconds")]
    pub usage_age_seconds: Option<f64>,
    #[serde(rename = "usageError")]
    pub usage_error: Option<String>,
    #[serde(rename = "usageRetryAt")]
    pub usage_retry_at: Option<String>,
    pub alias: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(rename = "loginExpiresAt")]
    pub login_expires_at: Option<String>,
}

impl AccountRow {
    pub fn display_name(&self) -> String {
        match &self.alias {
            Some(alias) => format!("{alias} · {}", self.email),
            None => self.email.clone(),
        }
    }

    pub fn shown_usage(&self) -> Option<(&Usage, bool)> {
        if let Some(u) = &self.usage {
            return Some((u, true));
        }
        self.last_good_usage.as_ref().map(|u| (u, false))
    }

    pub fn is_switchable(&self) -> bool {
        !self.active && self.usage_status != "api_key"
    }

    pub fn status_label(&self) -> Option<&'static str> {
        match self.usage_status.as_str() {
            "ok" => None,
            "token_expired" => Some("token expired"),
            "relogin_required" => Some("login again"),
            "api_key" => Some("API key"),
            "keychain_unavailable" => Some("keychain locked"),
            "foreign_credential" => Some("foreign credential"),
            "no_credentials" => Some("no credentials"),
            "unavailable" => Some("usage unavailable"),
            _ => Some("unknown"),
        }
    }

    pub fn worst_window_pct(&self) -> f64 {
        self.shown_usage()
            .map(|(u, _)| u.windows().iter().map(|(_, w)| w.pct).fold(0.0, f64::max))
            .unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Usage {
    #[serde(rename = "fiveHour")]
    pub five_hour: Option<Window>,
    #[serde(rename = "sevenDay")]
    pub seven_day: Option<Window>,
    #[serde(default)]
    pub scoped: Vec<ScopedWindow>,
    pub spend: Option<Spend>,
}

impl Usage {
    pub fn windows(&self) -> Vec<(String, &Window)> {
        let mut out = Vec::new();
        if let Some(w) = &self.five_hour {
            out.push(("5-hour".to_string(), w));
        }
        if let Some(w) = &self.seven_day {
            out.push(("7-day".to_string(), w));
        }
        for s in &self.scoped {
            out.push((format!("{} weekly", s.name), &s.window));
        }
        out
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Window {
    #[serde(default)]
    pub pct: f64,
    #[serde(rename = "resetsAt")]
    pub resets_at: Option<String>,
    #[serde(rename = "aheadOfPace", default)]
    pub ahead_of_pace: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScopedWindow {
    pub name: String,
    #[serde(flatten)]
    pub window: Window,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Spend {
    pub used: f64,
    pub limit: f64,
    #[serde(default)]
    pub pct: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
}

fn default_currency() -> String {
    "USD".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutoEvent {
    pub event: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub to: Option<serde_json::Value>,
    #[serde(default)]
    pub from: Option<serde_json::Value>,
}

impl AutoEvent {
    pub fn summary(&self) -> String {
        let who = |v: &Option<serde_json::Value>| -> String {
            v.as_ref()
                .and_then(|x| x.get("email").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| "?".to_string())
        };
        match self.event.as_str() {
            "switch" => format!(
                "switched {} → {} ({})",
                who(&self.from),
                who(&self.to),
                self.trigger.clone().unwrap_or_default()
            ),
            "no-switch" => format!(
                "holding: {}{}",
                self.reason.clone().unwrap_or_default(),
                self.detail.as_ref().map(|d| format!(" · {d}")).unwrap_or_default()
            ),
            "all-exhausted" => "every account is exhausted".to_string(),
            "error" => format!("error: {}", self.message.clone().unwrap_or_default()),
            "account-quarantined" => "an account was quarantined (dead token)".to_string(),
            other => other.to_string(),
        }
    }
}
