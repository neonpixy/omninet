import COmnideaFFI
import Foundation

// MARK: - Community Operations

/// Kingdom community operations — all state flows as JSON through Rust.
public enum KingdomCommunities {

    /// Create a new community. `basis` is a JSON string like `"Interest"`.
    public static func create(name: String, basis: String) throws -> String {
        guard let json = divi_kingdom_community_new(name, basis) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create community")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a member to a community. Returns updated community JSON.
    public static func addMember(communityJSON: String, pubkey: String, sponsor: String? = nil) throws -> String {
        guard let json = divi_kingdom_community_add_member(communityJSON, pubkey, sponsor) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add member")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Remove a member from a community. Returns updated community JSON.
    public static func removeMember(communityJSON: String, pubkey: String) throws -> String {
        guard let json = divi_kingdom_community_remove_member(communityJSON, pubkey) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove member")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Update a member's role. `role` is a JSON string like `"Steward"`.
    public static func updateRole(communityJSON: String, pubkey: String, role: String) throws -> String {
        guard let json = divi_kingdom_community_update_role(communityJSON, pubkey, role) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update role")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Activate a forming community. Returns updated community JSON.
    public static func activate(communityJSON: String) throws -> String {
        guard let json = divi_kingdom_community_activate(communityJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to activate community")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Proposal Operations

public enum KingdomProposals {

    /// Create a new proposal. `decidingBody` is JSON like `{"Community":"uuid..."}`.
    public static func create(author: String, decidingBody: String, title: String, body: String) throws -> String {
        guard let json = divi_kingdom_proposal_new(author, decidingBody, title, body) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create proposal")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a vote to a proposal. `voteJSON` is a JSON Vote.
    public static func addVote(proposalJSON: String, voteJSON: String) throws -> String {
        guard let json = divi_kingdom_proposal_add_vote(proposalJSON, voteJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add vote")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Open voting on a proposal. Returns updated proposal JSON.
    public static func openVoting(proposalJSON: String, closesAt: Date) throws -> String {
        let timestamp = Int64(closesAt.timeIntervalSince1970)
        guard let json = divi_kingdom_proposal_open_voting(proposalJSON, timestamp) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to open voting")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Compute the vote tally for a proposal. Returns JSON VoteTally.
    public static func tally(proposalJSON: String, eligibleVoters: UInt32) throws -> String {
        guard let json = divi_kingdom_proposal_tally(proposalJSON, eligibleVoters) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute tally")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Vote & Application Helpers

public enum KingdomVotes {

    /// Create a vote. `position` is JSON like `"Support"`.
    public static func create(voter: String, proposalId: String, position: String) throws -> String {
        guard let json = divi_kingdom_vote_new(voter, proposalId, position) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create vote")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

public enum KingdomApplications {

    /// Create a membership application.
    public static func create(communityId: String, applicant: String, statement: String) throws -> String {
        guard let json = divi_kingdom_application_new(communityId, applicant, statement) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create application")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Approve an application. Returns updated application JSON.
    public static func approve(applicationJSON: String, reviewer: String) throws -> String {
        guard let json = divi_kingdom_application_approve(applicationJSON, reviewer) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to approve application")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Reject an application. Returns updated application JSON.
    public static func reject(applicationJSON: String, reviewer: String, reason: String) throws -> String {
        guard let json = divi_kingdom_application_reject(applicationJSON, reviewer, reason) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to reject application")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
