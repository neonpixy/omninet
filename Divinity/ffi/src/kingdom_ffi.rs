use std::ffi::c_char;

use kingdom::{
    Assembly, AssemblyRecord, AssemblyType, Appeal, AppealGround,
    AppointmentSource, Challenge, ChallengeResponse, ChallengeTarget, ChallengeType,
    Charter, Community, CommunityBasis, CommunityRole, ConvocationTrigger,
    Consortium, ConsortiumCharter, ConsortiumMember,
    DecidingBody, DecisionOutcome, Delegate, DelegateRecall,
    DiscussionPost, Dispute, DisputeContext, DisputeType,
    DissolutionRecord, FederationAgreement, FederationRegistry, FederationScope,
    Mandate, MandateDecision, MembershipApplication,
    Proposal, ProposalOutcome, ProposalType, Resolution,
    Union, UnionType, Vote, VotePosition,
};

use crate::helpers::{c_str_to_str, json_to_c, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Community — JSON round-trip (deserialize, mutate, re-serialize)
// ===================================================================

/// Create a new community.
///
/// `basis_json` is a JSON string for CommunityBasis (e.g. `"Interest"`).
/// Returns JSON (Community). Caller must free via `divi_free_string`.
///
/// # Safety
/// `name` and `basis_json` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_new(
    name: *const c_char,
    basis_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_kingdom_community_new: invalid name");
        return std::ptr::null_mut();
    };

    let Some(basis_str) = c_str_to_str(basis_json) else {
        set_last_error("divi_kingdom_community_new: invalid basis_json");
        return std::ptr::null_mut();
    };

    let basis: CommunityBasis = match serde_json::from_str(basis_str) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let community = Community::new(name_str, basis);
    json_to_c(&community)
}

/// Add a member to a community.
///
/// Takes community JSON, returns modified community JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `sponsor` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_add_member(
    community_json: *const c_char,
    pubkey: *const c_char,
    sponsor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_add_member: invalid community_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_community_add_member: invalid pubkey");
        return std::ptr::null_mut();
    };

    let sponsor_opt = c_str_to_str(sponsor).map(String::from);

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_add_member: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = community.add_member(pk, sponsor_opt) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

/// Remove a member from a community.
///
/// Takes community JSON, returns modified community JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_remove_member(
    community_json: *const c_char,
    pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_remove_member: invalid community_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_community_remove_member: invalid pubkey");
        return std::ptr::null_mut();
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_remove_member: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = community.remove_member(pk) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

/// Update a member's role in a community.
///
/// `role_json` is a JSON CommunityRole (e.g. `"Steward"`).
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_update_role(
    community_json: *const c_char,
    pubkey: *const c_char,
    role_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_update_role: invalid community_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_community_update_role: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(role_str) = c_str_to_str(role_json) else {
        set_last_error("divi_kingdom_community_update_role: invalid role_json");
        return std::ptr::null_mut();
    };

    let role: CommunityRole = match serde_json::from_str(role_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_update_role: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_update_role: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = community.update_member_role(pk, role) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

/// Activate a forming community.
///
/// # Safety
/// `community_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_activate(
    community_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_activate: invalid community_json");
        return std::ptr::null_mut();
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_activate: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = community.activate() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

// ===================================================================
// Proposal — JSON round-trip
// ===================================================================

/// Create a new proposal.
///
/// `deciding_body_json` is a JSON DecidingBody (e.g. `{"Community":"uuid..."}`).
/// Returns JSON (Proposal). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_new(
    author: *const c_char,
    deciding_body_json: *const c_char,
    title: *const c_char,
    body: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_kingdom_proposal_new: invalid author");
        return std::ptr::null_mut();
    };

    let Some(db_str) = c_str_to_str(deciding_body_json) else {
        set_last_error("divi_kingdom_proposal_new: invalid deciding_body_json");
        return std::ptr::null_mut();
    };

    let Some(title_str) = c_str_to_str(title) else {
        set_last_error("divi_kingdom_proposal_new: invalid title");
        return std::ptr::null_mut();
    };

    let Some(body_str) = c_str_to_str(body) else {
        set_last_error("divi_kingdom_proposal_new: invalid body");
        return std::ptr::null_mut();
    };

    let deciding_body: DecidingBody = match serde_json::from_str(db_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let proposal = Proposal::new(author_str, deciding_body, title_str, body_str);
    json_to_c(&proposal)
}

/// Add a vote to a proposal.
///
/// `vote_json` is a JSON Vote. Returns modified proposal JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_add_vote(
    proposal_json: *const c_char,
    vote_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_add_vote: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let Some(vj) = c_str_to_str(vote_json) else {
        set_last_error("divi_kingdom_proposal_add_vote: invalid vote_json");
        return std::ptr::null_mut();
    };

    let mut proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_add_vote: {e}"));
            return std::ptr::null_mut();
        }
    };

    let vote: Vote = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_add_vote: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = proposal.add_vote(vote) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&proposal)
}

/// Open voting on a proposal. `closes_at` is a Unix timestamp (seconds).
///
/// # Safety
/// `proposal_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_open_voting(
    proposal_json: *const c_char,
    closes_at: i64,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_open_voting: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let mut proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_open_voting: {e}"));
            return std::ptr::null_mut();
        }
    };

    let closes = chrono::DateTime::from_timestamp(closes_at, 0)
        .unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(7));

    if let Err(e) = proposal.open_voting(closes) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&proposal)
}

/// Compute the vote tally for a proposal.
///
/// Returns JSON (VoteTally). Caller must free via `divi_free_string`.
///
/// # Safety
/// `proposal_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_tally(
    proposal_json: *const c_char,
    eligible_voters: u32,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_tally: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_tally: {e}"));
            return std::ptr::null_mut();
        }
    };

    let tally = proposal.tally(eligible_voters);
    json_to_c(&tally)
}

// ===================================================================
// Vote — pure constructor
// ===================================================================

/// Create a vote.
///
/// `position_json` is a JSON VotePosition (e.g. `"Support"`).
/// Returns JSON (Vote). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `proposal_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_vote_new(
    voter: *const c_char,
    proposal_id: *const c_char,
    position_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(voter_str) = c_str_to_str(voter) else {
        set_last_error("divi_kingdom_vote_new: invalid voter");
        return std::ptr::null_mut();
    };

    let Some(pid_str) = c_str_to_str(proposal_id) else {
        set_last_error("divi_kingdom_vote_new: invalid proposal_id");
        return std::ptr::null_mut();
    };

    let Some(pos_str) = c_str_to_str(position_json) else {
        set_last_error("divi_kingdom_vote_new: invalid position_json");
        return std::ptr::null_mut();
    };

    let pid = match uuid::Uuid::parse_str(pid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_vote_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let position: VotePosition = match serde_json::from_str(pos_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_vote_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let vote = Vote::new(voter_str, pid, position);
    json_to_c(&vote)
}

// ===================================================================
// Membership Application — JSON round-trip
// ===================================================================

/// Create a membership application.
///
/// Returns JSON (MembershipApplication). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `community_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_application_new(
    community_id: *const c_char,
    applicant_pubkey: *const c_char,
    statement: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_application_new: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(applicant_pubkey) else {
        set_last_error("divi_kingdom_application_new: invalid applicant_pubkey");
        return std::ptr::null_mut();
    };

    let Some(stmt) = c_str_to_str(statement) else {
        set_last_error("divi_kingdom_application_new: invalid statement");
        return std::ptr::null_mut();
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_application_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let app = MembershipApplication::new(cid, pk, stmt);
    json_to_c(&app)
}

/// Approve a membership application.
///
/// Takes application JSON, returns modified application JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_application_approve(
    app_json: *const c_char,
    reviewer: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(app_json) else {
        set_last_error("divi_kingdom_application_approve: invalid app_json");
        return std::ptr::null_mut();
    };

    let Some(rev) = c_str_to_str(reviewer) else {
        set_last_error("divi_kingdom_application_approve: invalid reviewer");
        return std::ptr::null_mut();
    };

    let mut app: MembershipApplication = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_application_approve: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = app.approve(rev) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&app)
}

/// Reject a membership application.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_application_reject(
    app_json: *const c_char,
    reviewer: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(app_json) else {
        set_last_error("divi_kingdom_application_reject: invalid app_json");
        return std::ptr::null_mut();
    };

    let Some(rev) = c_str_to_str(reviewer) else {
        set_last_error("divi_kingdom_application_reject: invalid reviewer");
        return std::ptr::null_mut();
    };

    let Some(rsn) = c_str_to_str(reason) else {
        set_last_error("divi_kingdom_application_reject: invalid reason");
        return std::ptr::null_mut();
    };

    let mut app: MembershipApplication = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_application_reject: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = app.reject(rev, rsn) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&app)
}

// ===================================================================
// Community — additional lifecycle (add_founder, go_dormant, dissolve)
// ===================================================================

/// Add a founding member to a community.
///
/// Takes community JSON and a pubkey. Returns modified community JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_add_founder(
    community_json: *const c_char,
    pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_add_founder: invalid community_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_community_add_founder: invalid pubkey");
        return std::ptr::null_mut();
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_add_founder: {e}"));
            return std::ptr::null_mut();
        }
    };

    community.add_founder(pk);
    json_to_c(&community)
}

/// Transition an active community to dormant.
///
/// Takes community JSON. Returns modified community JSON.
///
/// # Safety
/// `community_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_go_dormant(
    community_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_go_dormant: invalid community_json");
        return std::ptr::null_mut();
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_go_dormant: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = community.go_dormant() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

/// Dissolution state machine for a community.
///
/// `stage`: 0 = begin dissolution, 1 = complete dissolution, 2 = reactivate from dormant.
/// Returns modified community JSON.
///
/// # Safety
/// `community_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_community_dissolve(
    community_json: *const c_char,
    stage: u8,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(community_json) else {
        set_last_error("divi_kingdom_community_dissolve: invalid community_json");
        return std::ptr::null_mut();
    };

    let mut community: Community = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_community_dissolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    let result = match stage {
        0 => community.begin_dissolution(),
        1 => community.dissolve(),
        2 => community.reactivate(),
        _ => {
            set_last_error("divi_kingdom_community_dissolve: stage must be 0 (begin), 1 (dissolve), or 2 (reactivate)");
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = result {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&community)
}

// ===================================================================
// Charter — JSON round-trip
// ===================================================================

/// Create a new charter.
///
/// Returns JSON (Charter). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `community_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_charter_new(
    community_id: *const c_char,
    name: *const c_char,
    purpose: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_charter_new: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_kingdom_charter_new: invalid name");
        return std::ptr::null_mut();
    };

    let Some(purpose_str) = c_str_to_str(purpose) else {
        set_last_error("divi_kingdom_charter_new: invalid purpose");
        return std::ptr::null_mut();
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_charter_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let charter = Charter::new(cid, name_str, purpose_str);
    json_to_c(&charter)
}

/// Sign a charter with a pubkey and signature.
///
/// Takes charter JSON, returns modified charter JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_charter_sign(
    charter_json: *const c_char,
    pubkey: *const c_char,
    signature: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(charter_json) else {
        set_last_error("divi_kingdom_charter_sign: invalid charter_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_charter_sign: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(sig) = c_str_to_str(signature) else {
        set_last_error("divi_kingdom_charter_sign: invalid signature");
        return std::ptr::null_mut();
    };

    let mut charter: Charter = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_charter_sign: {e}"));
            return std::ptr::null_mut();
        }
    };

    charter.sign(pk, sig);
    json_to_c(&charter)
}

/// Create an amended version of a charter.
///
/// Takes charter JSON and a community_id UUID. Returns new charter JSON (version incremented).
///
/// # Safety
/// All C strings must be valid. `community_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_charter_amend(
    charter_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(json_str) = c_str_to_str(charter_json) else {
        set_last_error("divi_kingdom_charter_amend: invalid charter_json");
        return std::ptr::null_mut();
    };

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_charter_amend: invalid community_id");
        return std::ptr::null_mut();
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_charter_amend: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let charter: Charter = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_charter_amend: {e}"));
            return std::ptr::null_mut();
        }
    };

    let amended = charter.amend(cid);
    json_to_c(&amended)
}

// ===================================================================
// Proposal — additional lifecycle
// ===================================================================

/// Open discussion on a proposal (Draft -> Discussion).
///
/// Takes proposal JSON. Returns modified proposal JSON.
///
/// # Safety
/// `proposal_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_open_discussion(
    proposal_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_open_discussion: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let mut proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_open_discussion: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = proposal.open_discussion() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&proposal)
}

/// Resolve a proposal with an outcome.
///
/// Takes proposal JSON and outcome JSON. Returns modified proposal JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_resolve(
    proposal_json: *const c_char,
    outcome_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_resolve: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let Some(oj) = c_str_to_str(outcome_json) else {
        set_last_error("divi_kingdom_proposal_resolve: invalid outcome_json");
        return std::ptr::null_mut();
    };

    let mut proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_resolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    let outcome: ProposalOutcome = match serde_json::from_str(oj) {
        Ok(o) => o,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_resolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = proposal.resolve(outcome) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&proposal)
}

/// Withdraw a proposal.
///
/// Takes proposal JSON. Returns modified proposal JSON.
///
/// # Safety
/// `proposal_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_proposal_withdraw(
    proposal_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_kingdom_proposal_withdraw: invalid proposal_json");
        return std::ptr::null_mut();
    };

    let mut proposal: Proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_proposal_withdraw: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = proposal.withdraw() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&proposal)
}

// ===================================================================
// Discussion — pure constructor
// ===================================================================

/// Create a new discussion post.
///
/// `reply_to` may be null (top-level post) or a UUID string (reply).
/// Returns JSON (DiscussionPost). Caller must free via `divi_free_string`.
///
/// # Safety
/// `author` and `content` must be valid C strings. `reply_to` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_discussion_post_new(
    author: *const c_char,
    content: *const c_char,
    reply_to: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_kingdom_discussion_post_new: invalid author");
        return std::ptr::null_mut();
    };

    let Some(content_str) = c_str_to_str(content) else {
        set_last_error("divi_kingdom_discussion_post_new: invalid content");
        return std::ptr::null_mut();
    };

    let post = if let Some(reply_str) = c_str_to_str(reply_to) {
        let reply_id = match uuid::Uuid::parse_str(reply_str) {
            Ok(u) => u,
            Err(e) => {
                set_last_error(format!("divi_kingdom_discussion_post_new: invalid reply_to UUID: {e}"));
                return std::ptr::null_mut();
            }
        };
        DiscussionPost::reply(author_str, content_str, reply_id)
    } else {
        DiscussionPost::new(author_str, content_str)
    };

    json_to_c(&post)
}

// ===================================================================
// Mandate & Delegation — JSON round-trip
// ===================================================================

/// Create a new mandate.
///
/// `preset`: 0 = empty (custom), 1 = standard, 2 = limited.
/// `delegate_pubkey`, `community_id`, `consortium_id` are required strings.
/// Returns JSON (Mandate). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. UUID strings must be valid UUIDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_mandate_new(
    delegate_pubkey: *const c_char,
    community_id: *const c_char,
    consortium_id: *const c_char,
    preset: u8,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(delegate_pubkey) else {
        set_last_error("divi_kingdom_mandate_new: invalid delegate_pubkey");
        return std::ptr::null_mut();
    };

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_mandate_new: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(sid_str) = c_str_to_str(consortium_id) else {
        set_last_error("divi_kingdom_mandate_new: invalid consortium_id");
        return std::ptr::null_mut();
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_mandate_new: invalid community UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let sid = match uuid::Uuid::parse_str(sid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_mandate_new: invalid consortium UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mandate = match preset {
        0 => Mandate::new(pk, cid, sid),
        1 => Mandate::standard(pk, cid, sid),
        2 => Mandate::limited(pk, cid, sid),
        _ => {
            set_last_error("divi_kingdom_mandate_new: preset must be 0 (custom), 1 (standard), or 2 (limited)");
            return std::ptr::null_mut();
        }
    };

    json_to_c(&mandate)
}

/// Check whether a mandate authorizes a given decision type.
///
/// `proposal_type_json` is a JSON ProposalType (e.g. `"Standard"`).
/// Returns: 0 = Authorized, 1 = RequiresConsultation, 2 = Prohibited, -1 = error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_mandate_can_decide(
    mandate_json: *const c_char,
    proposal_type_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(mj) = c_str_to_str(mandate_json) else {
        set_last_error("divi_kingdom_mandate_can_decide: invalid mandate_json");
        return -1;
    };

    let Some(ptj) = c_str_to_str(proposal_type_json) else {
        set_last_error("divi_kingdom_mandate_can_decide: invalid proposal_type_json");
        return -1;
    };

    let mandate: Mandate = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_kingdom_mandate_can_decide: {e}"));
            return -1;
        }
    };

    let pt: ProposalType = match serde_json::from_str(ptj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_kingdom_mandate_can_decide: {e}"));
            return -1;
        }
    };

    match mandate.can_decide(&pt) {
        MandateDecision::Authorized => 0,
        MandateDecision::RequiresConsultation => 1,
        MandateDecision::Prohibited => 2,
    }
}

/// Create a new delegate.
///
/// `appointment_json` is a JSON AppointmentSource.
/// `mandate_json` is a JSON Mandate.
/// Returns JSON (Delegate). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. UUID strings must be valid UUIDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_delegate_new(
    pubkey: *const c_char,
    community_id: *const c_char,
    consortium_id: *const c_char,
    mandate_json: *const c_char,
    appointment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_delegate_new: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_delegate_new: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(sid_str) = c_str_to_str(consortium_id) else {
        set_last_error("divi_kingdom_delegate_new: invalid consortium_id");
        return std::ptr::null_mut();
    };

    let Some(mj) = c_str_to_str(mandate_json) else {
        set_last_error("divi_kingdom_delegate_new: invalid mandate_json");
        return std::ptr::null_mut();
    };

    let Some(aj) = c_str_to_str(appointment_json) else {
        set_last_error("divi_kingdom_delegate_new: invalid appointment_json");
        return std::ptr::null_mut();
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_delegate_new: invalid community UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let sid = match uuid::Uuid::parse_str(sid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_delegate_new: invalid consortium UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mandate: Mandate = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_kingdom_delegate_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let appointment: AppointmentSource = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_delegate_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let delegate = Delegate::new(pk, cid, sid, mandate, appointment);
    json_to_c(&delegate)
}

/// Create a recall motion against a delegate.
///
/// `delegate_id` and `community_id` are UUID strings.
/// `signatures_required` is the number of signatures needed to trigger the recall.
/// Returns JSON (DelegateRecall). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. UUID strings must be valid UUIDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_recall_new(
    delegate_id: *const c_char,
    delegate_pubkey: *const c_char,
    community_id: *const c_char,
    reason: *const c_char,
    signatures_required: u32,
) -> *mut c_char {
    clear_last_error();

    let Some(did_str) = c_str_to_str(delegate_id) else {
        set_last_error("divi_kingdom_recall_new: invalid delegate_id");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(delegate_pubkey) else {
        set_last_error("divi_kingdom_recall_new: invalid delegate_pubkey");
        return std::ptr::null_mut();
    };

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_recall_new: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error("divi_kingdom_recall_new: invalid reason");
        return std::ptr::null_mut();
    };

    let did = match uuid::Uuid::parse_str(did_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_recall_new: invalid delegate UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_recall_new: invalid community UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let recall = DelegateRecall::new(did, pk, cid, reason_str, signatures_required);
    json_to_c(&recall)
}

/// Add a signature to a recall motion.
///
/// Takes recall JSON, returns modified recall JSON.
/// If the signature threshold is met, the recall auto-transitions to Triggered.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_recall_add_signature(
    recall_json: *const c_char,
    pubkey: *const c_char,
    signature: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(recall_json) else {
        set_last_error("divi_kingdom_recall_add_signature: invalid recall_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_kingdom_recall_add_signature: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(sig) = c_str_to_str(signature) else {
        set_last_error("divi_kingdom_recall_add_signature: invalid signature");
        return std::ptr::null_mut();
    };

    let mut recall: DelegateRecall = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_kingdom_recall_add_signature: {e}"));
            return std::ptr::null_mut();
        }
    };

    recall.add_signature(pk, sig);
    json_to_c(&recall)
}

// ===================================================================
// Consortium — JSON round-trip
// ===================================================================

/// Create a new consortium.
///
/// `charter_json` is a JSON ConsortiumCharter.
/// Returns JSON (Consortium). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_consortium_new(
    name: *const c_char,
    purpose: *const c_char,
    charter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_kingdom_consortium_new: invalid name");
        return std::ptr::null_mut();
    };

    let Some(purpose_str) = c_str_to_str(purpose) else {
        set_last_error("divi_kingdom_consortium_new: invalid purpose");
        return std::ptr::null_mut();
    };

    let Some(cj) = c_str_to_str(charter_json) else {
        set_last_error("divi_kingdom_consortium_new: invalid charter_json");
        return std::ptr::null_mut();
    };

    let charter: ConsortiumCharter = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_consortium_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let consortium = Consortium::new(name_str, purpose_str, charter);
    json_to_c(&consortium)
}

/// Add a member community to a consortium.
///
/// `member_json` is a JSON ConsortiumMember.
/// Returns modified consortium JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_consortium_add_member(
    consortium_json: *const c_char,
    member_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(consortium_json) else {
        set_last_error("divi_kingdom_consortium_add_member: invalid consortium_json");
        return std::ptr::null_mut();
    };

    let Some(mj) = c_str_to_str(member_json) else {
        set_last_error("divi_kingdom_consortium_add_member: invalid member_json");
        return std::ptr::null_mut();
    };

    let mut consortium: Consortium = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_consortium_add_member: {e}"));
            return std::ptr::null_mut();
        }
    };

    let member: ConsortiumMember = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_kingdom_consortium_add_member: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = consortium.add_member(member) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&consortium)
}

/// Remove a member community from a consortium by community_id.
///
/// Returns modified consortium JSON.
///
/// # Safety
/// All C strings must be valid. `community_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_consortium_remove_member(
    consortium_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(consortium_json) else {
        set_last_error("divi_kingdom_consortium_remove_member: invalid consortium_json");
        return std::ptr::null_mut();
    };

    let Some(cid_str) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_consortium_remove_member: invalid community_id");
        return std::ptr::null_mut();
    };

    let mut consortium: Consortium = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_consortium_remove_member: {e}"));
            return std::ptr::null_mut();
        }
    };

    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_consortium_remove_member: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = consortium.remove_member(&cid) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&consortium)
}

// ===================================================================
// Assembly — JSON round-trip
// ===================================================================

/// Create a new assembly (convocation).
///
/// `type_json` is a JSON AssemblyType (e.g. `"CommunityAssembly"`).
/// `trigger_json` is a JSON ConvocationTrigger (e.g. `"Scheduled"`).
/// Returns JSON (Assembly). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_assembly_new(
    name: *const c_char,
    type_json: *const c_char,
    convened_by: *const c_char,
    purpose: *const c_char,
    trigger_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_kingdom_assembly_new: invalid name");
        return std::ptr::null_mut();
    };

    let Some(tj) = c_str_to_str(type_json) else {
        set_last_error("divi_kingdom_assembly_new: invalid type_json");
        return std::ptr::null_mut();
    };

    let Some(convener) = c_str_to_str(convened_by) else {
        set_last_error("divi_kingdom_assembly_new: invalid convened_by");
        return std::ptr::null_mut();
    };

    let Some(purpose_str) = c_str_to_str(purpose) else {
        set_last_error("divi_kingdom_assembly_new: invalid purpose");
        return std::ptr::null_mut();
    };

    let Some(trj) = c_str_to_str(trigger_json) else {
        set_last_error("divi_kingdom_assembly_new: invalid trigger_json");
        return std::ptr::null_mut();
    };

    let assembly_type: AssemblyType = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_assembly_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let trigger: ConvocationTrigger = match serde_json::from_str(trj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_assembly_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let assembly = Assembly::new(name_str, assembly_type, convener, purpose_str, trigger);
    json_to_c(&assembly)
}

/// Transition an assembly's lifecycle.
///
/// `action`: 0 = begin (Convened -> InProgress), 1 = pause, 2 = resume, 3 = conclude.
/// Returns modified assembly JSON.
///
/// # Safety
/// `assembly_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_assembly_transition(
    assembly_json: *const c_char,
    action: u8,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(assembly_json) else {
        set_last_error("divi_kingdom_assembly_transition: invalid assembly_json");
        return std::ptr::null_mut();
    };

    let mut assembly: Assembly = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_assembly_transition: {e}"));
            return std::ptr::null_mut();
        }
    };

    match action {
        0 => {
            if let Err(e) = assembly.begin() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        1 => assembly.pause(),
        2 => assembly.resume(),
        3 => {
            if let Err(e) = assembly.conclude() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        _ => {
            set_last_error("divi_kingdom_assembly_transition: action must be 0 (begin), 1 (pause), 2 (resume), or 3 (conclude)");
            return std::ptr::null_mut();
        }
    }

    json_to_c(&assembly)
}

/// Add a record to an assembly.
///
/// `record_json` is a JSON AssemblyRecord.
/// Returns modified assembly JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_assembly_add_record(
    assembly_json: *const c_char,
    record_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(assembly_json) else {
        set_last_error("divi_kingdom_assembly_add_record: invalid assembly_json");
        return std::ptr::null_mut();
    };

    let Some(rj) = c_str_to_str(record_json) else {
        set_last_error("divi_kingdom_assembly_add_record: invalid record_json");
        return std::ptr::null_mut();
    };

    let mut assembly: Assembly = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_assembly_add_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let record: AssemblyRecord = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_kingdom_assembly_add_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = assembly.add_record(record) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&assembly)
}

// ===================================================================
// Challenge — JSON round-trip
// ===================================================================

/// Create a new challenge.
///
/// `type_json` is a JSON ChallengeType (e.g. `"CoreViolation"`).
/// `target_json` is a JSON ChallengeTarget.
/// Returns JSON (Challenge). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_challenge_new(
    challenger: *const c_char,
    type_json: *const c_char,
    target_json: *const c_char,
    grounds: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(challenger_str) = c_str_to_str(challenger) else {
        set_last_error("divi_kingdom_challenge_new: invalid challenger");
        return std::ptr::null_mut();
    };

    let Some(tj) = c_str_to_str(type_json) else {
        set_last_error("divi_kingdom_challenge_new: invalid type_json");
        return std::ptr::null_mut();
    };

    let Some(tgj) = c_str_to_str(target_json) else {
        set_last_error("divi_kingdom_challenge_new: invalid target_json");
        return std::ptr::null_mut();
    };

    let Some(grounds_str) = c_str_to_str(grounds) else {
        set_last_error("divi_kingdom_challenge_new: invalid grounds");
        return std::ptr::null_mut();
    };

    let challenge_type: ChallengeType = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_challenge_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let target: ChallengeTarget = match serde_json::from_str(tgj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_challenge_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let challenge = Challenge::new(challenger_str, challenge_type, target, grounds_str);
    json_to_c(&challenge)
}

/// Respond to a challenge.
///
/// `response_json` is a JSON ChallengeResponse.
/// Returns modified challenge JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_challenge_respond(
    challenge_json: *const c_char,
    response_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(challenge_json) else {
        set_last_error("divi_kingdom_challenge_respond: invalid challenge_json");
        return std::ptr::null_mut();
    };

    let Some(rj) = c_str_to_str(response_json) else {
        set_last_error("divi_kingdom_challenge_respond: invalid response_json");
        return std::ptr::null_mut();
    };

    let mut challenge: Challenge = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_challenge_respond: {e}"));
            return std::ptr::null_mut();
        }
    };

    let response: ChallengeResponse = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_kingdom_challenge_respond: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = challenge.respond(response) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&challenge)
}

/// Resolve a challenge by upholding or dismissing it.
///
/// `upheld`: 0 = dismiss, 1 = uphold.
/// Returns modified challenge JSON.
///
/// # Safety
/// `challenge_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_challenge_resolve(
    challenge_json: *const c_char,
    upheld: u8,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(challenge_json) else {
        set_last_error("divi_kingdom_challenge_resolve: invalid challenge_json");
        return std::ptr::null_mut();
    };

    let mut challenge: Challenge = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_challenge_resolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    let result = match upheld {
        0 => challenge.dismiss(),
        1 => challenge.uphold(),
        _ => {
            set_last_error("divi_kingdom_challenge_resolve: upheld must be 0 (dismiss) or 1 (uphold)");
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = result {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&challenge)
}

// ===================================================================
// Adjudication — JSON round-trip
// ===================================================================

/// Create a new dispute.
///
/// `type_json` is a JSON DisputeType (e.g. `"ContractBreach"`).
/// `context_json` is a JSON DisputeContext (e.g. `"Interpersonal"`).
/// Returns JSON (Dispute). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_dispute_new(
    complainant: *const c_char,
    respondent: *const c_char,
    type_json: *const c_char,
    context_json: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(comp) = c_str_to_str(complainant) else {
        set_last_error("divi_kingdom_dispute_new: invalid complainant");
        return std::ptr::null_mut();
    };

    let Some(resp) = c_str_to_str(respondent) else {
        set_last_error("divi_kingdom_dispute_new: invalid respondent");
        return std::ptr::null_mut();
    };

    let Some(tj) = c_str_to_str(type_json) else {
        set_last_error("divi_kingdom_dispute_new: invalid type_json");
        return std::ptr::null_mut();
    };

    let Some(cxj) = c_str_to_str(context_json) else {
        set_last_error("divi_kingdom_dispute_new: invalid context_json");
        return std::ptr::null_mut();
    };

    let Some(desc) = c_str_to_str(description) else {
        set_last_error("divi_kingdom_dispute_new: invalid description");
        return std::ptr::null_mut();
    };

    let dispute_type: DisputeType = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_dispute_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let context: DisputeContext = match serde_json::from_str(cxj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_kingdom_dispute_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let dispute = Dispute::new(comp, resp, dispute_type, context, desc);
    json_to_c(&dispute)
}

/// Transition a dispute through its lifecycle.
///
/// `action`: 0 = require_response, 1 = advance_to_hearing, 2 = advance_to_resolution,
///           3 = resolve, 4 = dismiss, 5 = withdraw.
/// Returns modified dispute JSON.
///
/// # Safety
/// `dispute_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_dispute_transition(
    dispute_json: *const c_char,
    action: u8,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(dispute_json) else {
        set_last_error("divi_kingdom_dispute_transition: invalid dispute_json");
        return std::ptr::null_mut();
    };

    let mut dispute: Dispute = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_kingdom_dispute_transition: {e}"));
            return std::ptr::null_mut();
        }
    };

    match action {
        0 => {
            if let Err(e) = dispute.require_response() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        1 => {
            if let Err(e) = dispute.advance_to_hearing() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        2 => {
            if let Err(e) = dispute.advance_to_resolution() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        3 => {
            if let Err(e) = dispute.resolve() {
                set_last_error(e.to_string());
                return std::ptr::null_mut();
            }
        }
        4 => dispute.dismiss(),
        5 => dispute.withdraw(),
        _ => {
            set_last_error("divi_kingdom_dispute_transition: action must be 0-5");
            return std::ptr::null_mut();
        }
    }

    json_to_c(&dispute)
}

/// Create a new resolution for a dispute.
///
/// `decided_by_json` is a JSON array of adjudicator UUIDs.
/// `decision_json` is a JSON DecisionOutcome (e.g. `"ForComplainant"`).
/// Returns JSON (Resolution). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `dispute_id` must be a UUID string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_resolution_new(
    dispute_id: *const c_char,
    decided_by_json: *const c_char,
    decision_json: *const c_char,
    reasoning: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(did_str) = c_str_to_str(dispute_id) else {
        set_last_error("divi_kingdom_resolution_new: invalid dispute_id");
        return std::ptr::null_mut();
    };

    let Some(dbj) = c_str_to_str(decided_by_json) else {
        set_last_error("divi_kingdom_resolution_new: invalid decided_by_json");
        return std::ptr::null_mut();
    };

    let Some(dj) = c_str_to_str(decision_json) else {
        set_last_error("divi_kingdom_resolution_new: invalid decision_json");
        return std::ptr::null_mut();
    };

    let Some(reason_str) = c_str_to_str(reasoning) else {
        set_last_error("divi_kingdom_resolution_new: invalid reasoning");
        return std::ptr::null_mut();
    };

    let did = match uuid::Uuid::parse_str(did_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_resolution_new: invalid dispute UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let decided_by: Vec<uuid::Uuid> = match serde_json::from_str(dbj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_kingdom_resolution_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let decision: DecisionOutcome = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_kingdom_resolution_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let resolution = Resolution::new(did, decided_by, decision, reason_str);
    json_to_c(&resolution)
}

/// Create a new appeal of a resolution.
///
/// `grounds_json` is a JSON array of AppealGround (e.g. `["ProceduralError","Bias"]`).
/// Returns JSON (Appeal). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. UUID strings must be valid UUIDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_appeal_new(
    resolution_id: *const c_char,
    dispute_id: *const c_char,
    appellant: *const c_char,
    grounds_json: *const c_char,
    argument: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rid_str) = c_str_to_str(resolution_id) else {
        set_last_error("divi_kingdom_appeal_new: invalid resolution_id");
        return std::ptr::null_mut();
    };

    let Some(did_str) = c_str_to_str(dispute_id) else {
        set_last_error("divi_kingdom_appeal_new: invalid dispute_id");
        return std::ptr::null_mut();
    };

    let Some(appellant_str) = c_str_to_str(appellant) else {
        set_last_error("divi_kingdom_appeal_new: invalid appellant");
        return std::ptr::null_mut();
    };

    let Some(gj) = c_str_to_str(grounds_json) else {
        set_last_error("divi_kingdom_appeal_new: invalid grounds_json");
        return std::ptr::null_mut();
    };

    let Some(arg_str) = c_str_to_str(argument) else {
        set_last_error("divi_kingdom_appeal_new: invalid argument");
        return std::ptr::null_mut();
    };

    let rid = match uuid::Uuid::parse_str(rid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_appeal_new: invalid resolution UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let did = match uuid::Uuid::parse_str(did_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_appeal_new: invalid dispute UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let grounds: Vec<AppealGround> = match serde_json::from_str(gj) {
        Ok(g) => g,
        Err(e) => {
            set_last_error(format!("divi_kingdom_appeal_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let appeal = Appeal::new(rid, did, appellant_str, grounds, arg_str);
    json_to_c(&appeal)
}

// ===================================================================
// Union — JSON round-trip
// ===================================================================

/// Create a new union.
///
/// `type_json` is a JSON UnionType (e.g. `"Marriage"`, `"WorkerCooperative"`).
/// Returns JSON (Union). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_union_new(
    name: *const c_char,
    type_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_kingdom_union_new: invalid name");
        return std::ptr::null_mut();
    };

    let Some(tj) = c_str_to_str(type_json) else {
        set_last_error("divi_kingdom_union_new: invalid type_json");
        return std::ptr::null_mut();
    };

    let union_type: UnionType = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_kingdom_union_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let union_val = Union::new(name_str, union_type);
    json_to_c(&union_val)
}

/// Dissolve a union.
///
/// `record_json` is a JSON DissolutionRecord.
/// Returns modified union JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_union_dissolve(
    union_json: *const c_char,
    record_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(uj) = c_str_to_str(union_json) else {
        set_last_error("divi_kingdom_union_dissolve: invalid union_json");
        return std::ptr::null_mut();
    };

    let Some(rj) = c_str_to_str(record_json) else {
        set_last_error("divi_kingdom_union_dissolve: invalid record_json");
        return std::ptr::null_mut();
    };

    let mut union_val: Union = match serde_json::from_str(uj) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_kingdom_union_dissolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    let record: DissolutionRecord = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_kingdom_union_dissolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = union_val.dissolve(record) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&union_val)
}

// ===================================================================
// Federation Agreement — JSON round-trip
// ===================================================================

/// Propose a new federation agreement between two communities.
///
/// `scopes_json` is a JSON array of `FederationScope` values.
/// Returns JSON (FederationAgreement). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_propose(
    community_a: *const c_char,
    community_b: *const c_char,
    proposed_by: *const c_char,
    scopes_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(a_str) = c_str_to_str(community_a) else {
        set_last_error("divi_kingdom_federation_propose: invalid community_a");
        return std::ptr::null_mut();
    };

    let Some(b_str) = c_str_to_str(community_b) else {
        set_last_error("divi_kingdom_federation_propose: invalid community_b");
        return std::ptr::null_mut();
    };

    let Some(by_str) = c_str_to_str(proposed_by) else {
        set_last_error("divi_kingdom_federation_propose: invalid proposed_by");
        return std::ptr::null_mut();
    };

    let Some(sj) = c_str_to_str(scopes_json) else {
        set_last_error("divi_kingdom_federation_propose: invalid scopes_json");
        return std::ptr::null_mut();
    };

    let scopes: Vec<FederationScope> = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_propose: {e}"));
            return std::ptr::null_mut();
        }
    };

    let agreement = FederationAgreement::propose(a_str, b_str, by_str, scopes);
    json_to_c(&agreement)
}

/// Accept a proposed federation agreement (Proposed -> Active).
///
/// Returns modified agreement JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_accept(
    agreement_json: *const c_char,
    accepted_by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_accept: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let Some(by_str) = c_str_to_str(accepted_by) else {
        set_last_error("divi_kingdom_federation_accept: invalid accepted_by");
        return std::ptr::null_mut();
    };

    let mut agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_accept: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = agreement.accept(by_str) {
        set_last_error(format!("divi_kingdom_federation_accept: {e}"));
        return std::ptr::null_mut();
    }

    json_to_c(&agreement)
}

/// Suspend an active federation agreement (Active -> Suspended).
///
/// Returns modified agreement JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_suspend(
    agreement_json: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_suspend: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error("divi_kingdom_federation_suspend: invalid reason");
        return std::ptr::null_mut();
    };

    let mut agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_suspend: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = agreement.suspend(reason_str) {
        set_last_error(format!("divi_kingdom_federation_suspend: {e}"));
        return std::ptr::null_mut();
    }

    json_to_c(&agreement)
}

/// Reactivate a suspended federation agreement (Suspended -> Active).
///
/// Returns modified agreement JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// `agreement_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_reactivate(
    agreement_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_reactivate: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let mut agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_reactivate: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = agreement.reactivate() {
        set_last_error(format!("divi_kingdom_federation_reactivate: {e}"));
        return std::ptr::null_mut();
    }

    json_to_c(&agreement)
}

/// Withdraw from a federation agreement (Active|Suspended -> Withdrawn).
///
/// Returns modified agreement JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_withdraw(
    agreement_json: *const c_char,
    withdrawn_by: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_withdraw: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let Some(by_str) = c_str_to_str(withdrawn_by) else {
        set_last_error("divi_kingdom_federation_withdraw: invalid withdrawn_by");
        return std::ptr::null_mut();
    };

    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error("divi_kingdom_federation_withdraw: invalid reason");
        return std::ptr::null_mut();
    };

    let mut agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_withdraw: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = agreement.withdraw(by_str, reason_str) {
        set_last_error(format!("divi_kingdom_federation_withdraw: {e}"));
        return std::ptr::null_mut();
    }

    json_to_c(&agreement)
}

/// Get the status of a federation agreement.
///
/// Returns JSON (FederationStatus). Caller must free via `divi_free_string`.
///
/// # Safety
/// `agreement_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_status(
    agreement_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_status: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_status: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&agreement.status)
}

/// Check whether a federation agreement involves a specific community.
///
/// Returns `true` if the community is party to the agreement.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_involves(
    agreement_json: *const c_char,
    community_id: *const c_char,
) -> bool {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_involves: invalid agreement_json");
        return false;
    };

    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_federation_involves: invalid community_id");
        return false;
    };

    let agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_involves: {e}"));
            return false;
        }
    };

    agreement.involves(cid)
}

/// Get the partner community ID from a federation agreement.
///
/// Returns the partner's community ID string, or null if `community_id`
/// is not a party. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_partner_of(
    agreement_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_partner_of: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_federation_partner_of: invalid community_id");
        return std::ptr::null_mut();
    };

    let agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_partner_of: {e}"));
            return std::ptr::null_mut();
        }
    };

    match agreement.partner_of(cid) {
        Some(partner) => string_to_c(partner.to_string()),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Federation Registry — opaque pointer lifecycle
// ===================================================================

/// Create a new empty federation registry.
#[unsafe(no_mangle)]
pub extern "C" fn divi_kingdom_federation_registry_new() -> *mut FederationRegistry {
    clear_last_error();
    Box::into_raw(Box::new(FederationRegistry::new()))
}

/// Free a federation registry.
///
/// # Safety
/// `ptr` must have been returned by `divi_kingdom_federation_registry_new` or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_free(
    ptr: *mut FederationRegistry,
) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Register a federation agreement in the registry.
///
/// Returns JSON string of the agreement UUID on success.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry`. `agreement_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_register(
    ptr: *mut FederationRegistry,
    agreement_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_register: null pointer");
        return std::ptr::null_mut();
    }

    let Some(aj) = c_str_to_str(agreement_json) else {
        set_last_error("divi_kingdom_federation_registry_register: invalid agreement_json");
        return std::ptr::null_mut();
    };

    let agreement: FederationAgreement = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_registry_register: {e}"));
            return std::ptr::null_mut();
        }
    };

    let registry = unsafe { &mut *ptr };
    match registry.register(agreement) {
        Ok(id) => json_to_c(&id),
        Err(e) => {
            set_last_error(format!("divi_kingdom_federation_registry_register: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Check whether two communities are actively federated.
///
/// Returns 1 for true, 0 for false, -1 on error.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry`. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_is_federated(
    ptr: *const FederationRegistry,
    community_a: *const c_char,
    community_b: *const c_char,
) -> i32 {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_is_federated: null pointer");
        return -1;
    }

    let Some(a_str) = c_str_to_str(community_a) else {
        set_last_error("divi_kingdom_federation_registry_is_federated: invalid community_a");
        return -1;
    };

    let Some(b_str) = c_str_to_str(community_b) else {
        set_last_error("divi_kingdom_federation_registry_is_federated: invalid community_b");
        return -1;
    };

    let registry = unsafe { &*ptr };
    if registry.is_federated(a_str, b_str) { 1 } else { 0 }
}

/// List all communities actively federated with a given community.
///
/// Returns JSON array of community ID strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry`. `community_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_federated_with(
    ptr: *const FederationRegistry,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_federated_with: null pointer");
        return std::ptr::null_mut();
    }

    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_kingdom_federation_registry_federated_with: invalid community_id");
        return std::ptr::null_mut();
    };

    let registry = unsafe { &*ptr };
    let partners: Vec<&str> = registry.federated_with(cid);
    json_to_c(&partners)
}

/// Find a federation path between two communities via active links (BFS).
///
/// Returns JSON array of community ID strings (the path), or JSON `null`
/// if no path exists. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry`. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_path_between(
    ptr: *const FederationRegistry,
    from: *const c_char,
    to: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_path_between: null pointer");
        return std::ptr::null_mut();
    }

    let Some(from_str) = c_str_to_str(from) else {
        set_last_error("divi_kingdom_federation_registry_path_between: invalid from");
        return std::ptr::null_mut();
    };

    let Some(to_str) = c_str_to_str(to) else {
        set_last_error("divi_kingdom_federation_registry_path_between: invalid to");
        return std::ptr::null_mut();
    };

    let registry = unsafe { &*ptr };
    let path = registry.path_between(from_str, to_str);
    json_to_c(&path)
}

/// Get all active federation agreements from the registry.
///
/// Returns JSON array of FederationAgreement objects.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_all_active(
    ptr: *const FederationRegistry,
) -> *mut c_char {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_all_active: null pointer");
        return std::ptr::null_mut();
    }

    let registry = unsafe { &*ptr };
    let active: Vec<&FederationAgreement> = registry.all_active();
    json_to_c(&active)
}

/// Get the total number of agreements in the registry (any status).
///
/// Returns the count, or -1 on null pointer.
///
/// # Safety
/// `ptr` must be a valid `FederationRegistry` or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_kingdom_federation_registry_count(
    ptr: *const FederationRegistry,
) -> i32 {
    clear_last_error();

    if ptr.is_null() {
        set_last_error("divi_kingdom_federation_registry_count: null pointer");
        return -1;
    }

    let registry = unsafe { &*ptr };
    registry.total_agreements() as i32
}
