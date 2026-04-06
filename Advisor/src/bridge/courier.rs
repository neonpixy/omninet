//! Courier bridge — translates courier.* SkillCalls into DirectResults.
//!
//! Courier skills route through Equipment's Mail system, not Magic Actions.
//! All results are DirectResult — the caller is responsible for actually
//! sending through Equipment.

use chrono::Utc;
use uuid::Uuid;

use equipment::mail_types::{
    BulkSend, MailDraft, MailRecipient, MailRecipientEntry, RecipientRole,
};

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Courier mail skills.
///
/// All Courier operations produce DirectResults containing serialized
/// Equipment mail types. The caller wires these into the actual Mailbox.
pub struct CourierBridge;

impl ActionBridge for CourierBridge {
    fn program_prefix(&self) -> &str {
        "courier"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "courier.composeMail" => translate_compose_mail(call),
            "courier.addRecipients" => translate_add_recipients(call),
            "courier.send" => translate_send(call),
            "courier.schedule" => translate_schedule(call),
            "courier.createNewsletter" => translate_create_newsletter(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_compose_mail(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let subject = call.get_string("subject")?;
    let body = call.get_string("body")?;
    let thread_id = call.get_string_opt("thread_id");
    let in_reply_to = call
        .get_string_opt("in_reply_to")
        .map(|s| parse_uuid(&s))
        .transpose()?;

    let now = Utc::now();
    let draft = MailDraft {
        id: Uuid::new_v4(),
        recipients: Vec::new(),
        subject: subject.clone(),
        body: body.clone(),
        attachments: Vec::new(),
        thread_id,
        in_reply_to,
        created_at: now,
        updated_at: now,
    };

    let draft_json = serde_json::to_string(&draft).map_err(AdvisorError::from)?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!("Draft created: {subject}"))
            .with_data("draft_id", draft.id.to_string())
            .with_data("draft_json", draft_json),
    ))
}

fn translate_add_recipients(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let draft_id = call.get_string("draft_id")?;
    let recipients_json = call.get_string("recipients")?;

    // Parse the recipients array.
    let entries: Vec<RecipientEntry> = serde_json::from_str(&recipients_json).map_err(|e| {
        AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid recipients JSON: {e}"),
        }
    })?;

    let mail_entries: Vec<MailRecipientEntry> = entries
        .into_iter()
        .map(|e| MailRecipientEntry {
            recipient: MailRecipient {
                crown_id: e.crown_id,
                display_name: None,
            },
            role: parse_role(&e.role),
        })
        .collect();

    let entries_json = serde_json::to_string(&mail_entries).map_err(AdvisorError::from)?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Added {} recipients to draft {draft_id}",
            mail_entries.len()
        ))
        .with_data("draft_id", draft_id)
        .with_data("recipients_json", entries_json),
    ))
}

fn translate_send(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let draft_id = call.get_string("draft_id")?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!("Send requested for draft {draft_id}"))
            .with_data("draft_id", draft_id)
            .with_data("action", "send".to_string()),
    ))
}

fn translate_schedule(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let draft_id = call.get_string("draft_id")?;
    let send_at = call.get_string("send_at")?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Scheduled draft {draft_id} for {send_at}"
        ))
        .with_data("draft_id", draft_id)
        .with_data("send_at", send_at)
        .with_data("action", "schedule".to_string()),
    ))
}

fn translate_create_newsletter(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let subject = call.get_string("subject")?;
    let body = call.get_string("body")?;
    let recipients_json = call.get_string("recipients")?;

    let crown_ids: Vec<String> =
        serde_json::from_str(&recipients_json).map_err(|e| AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid recipients JSON: {e}"),
        })?;

    let recipients: Vec<MailRecipient> = crown_ids
        .into_iter()
        .map(|id| MailRecipient {
            crown_id: id,
            display_name: None,
        })
        .collect();

    let bulk = BulkSend {
        id: Uuid::new_v4(),
        template_subject: subject.clone(),
        template_body: body,
        recipients,
        created_at: Utc::now(),
    };

    let bulk_json = serde_json::to_string(&bulk).map_err(AdvisorError::from)?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Newsletter created: {subject} ({} recipients)",
            bulk.recipients.len()
        ))
        .with_data("bulk_send_id", bulk.id.to_string())
        .with_data("bulk_send_json", bulk_json),
    ))
}

// ── Internal types ──────────────────────────────────────────────────

/// Lightweight recipient entry for JSON parsing from skill arguments.
#[derive(serde::Deserialize)]
struct RecipientEntry {
    crown_id: String,
    #[serde(default = "default_role")]
    role: String,
}

fn default_role() -> String {
    "to".into()
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "courier".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

fn parse_role(s: &str) -> RecipientRole {
    match s {
        "cc" => RecipientRole::Cc,
        "bcc" => RecipientRole::Bcc,
        _ => RecipientRole::To,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_mail_produces_direct_result() {
        let call = SkillCall::new("c1", "courier.composeMail")
            .with_argument("subject", serde_json::Value::String("Hello".into()))
            .with_argument("body", serde_json::Value::String("Hi there!".into()));

        let result = CourierBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref sr) => {
                assert!(sr.success);
                assert!(sr.data.contains_key("draft_id"));
                assert!(sr.data.contains_key("draft_json"));
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn send_produces_direct_result() {
        let call = SkillCall::new("c1", "courier.send")
            .with_argument("draft_id", serde_json::Value::String(Uuid::new_v4().to_string()));

        let result = CourierBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn schedule_produces_direct_result() {
        let call = SkillCall::new("c1", "courier.schedule")
            .with_argument("draft_id", serde_json::Value::String(Uuid::new_v4().to_string()))
            .with_argument("send_at", serde_json::Value::String("2026-03-15T10:00:00Z".into()));

        let result = CourierBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn create_newsletter_produces_direct_result() {
        let call = SkillCall::new("c1", "courier.createNewsletter")
            .with_argument("subject", serde_json::Value::String("News".into()))
            .with_argument("body", serde_json::Value::String("Updates".into()))
            .with_argument(
                "recipients",
                serde_json::Value::String(r#"["cpub_alice","cpub_bob"]"#.into()),
            );

        let result = CourierBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref sr) => {
                assert!(sr.success);
                assert!(sr.data.contains_key("bulk_send_id"));
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn add_recipients_produces_direct_result() {
        let call = SkillCall::new("c1", "courier.addRecipients")
            .with_argument("draft_id", serde_json::Value::String(Uuid::new_v4().to_string()))
            .with_argument(
                "recipients",
                serde_json::Value::String(r#"[{"crown_id":"cpub_alice","role":"to"}]"#.into()),
            );

        let result = CourierBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }
}
