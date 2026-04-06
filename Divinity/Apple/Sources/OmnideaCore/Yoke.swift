import COmnideaFFI
import Foundation

// MARK: - YokeLink

/// Provenance links between entities -- the binding thread of history.
///
/// All operations are stateless JSON round-trips through Rust.
/// Relation types are JSON strings like `"DerivedFrom"` or `{"Custom":"blocks"}`.
public enum YokeLink {

    /// Create a new link between two entities.
    ///
    /// - Parameters:
    ///   - source: The originating entity ID.
    ///   - target: The destination entity ID.
    ///   - relationTypeJSON: A JSON `RelationType` (e.g. `"DerivedFrom"`, `{"Custom":"blocks"}`).
    ///   - author: The public key of the link author.
    /// - Returns: JSON string representing the created `YokeLink`.
    public static func new(
        source: String,
        target: String,
        relationTypeJSON: String,
        author: String
    ) throws -> String {
        guard let json = divi_yoke_link_new(source, target, relationTypeJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create YokeLink")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add metadata to a link.
    ///
    /// - Parameters:
    ///   - linkJSON: The existing link as JSON.
    ///   - key: The metadata key.
    ///   - valueJSON: A JSON `x::Value` (e.g. `{"String":"2.0"}` or `{"Bool":true}`).
    /// - Returns: Modified link JSON with the metadata added.
    public static func withMetadata(
        linkJSON: String,
        key: String,
        valueJSON: String
    ) throws -> String {
        guard let json = divi_yoke_link_with_metadata(linkJSON, key, valueJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add metadata to YokeLink")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - YokeVersionTag

/// Version tags -- named snapshots of an .idea at a point in time.
///
/// Tags live on branches (default "main") and reference a `VectorClock` snapshot.
public enum YokeVersionTag {

    /// Create a new version tag on the "main" branch.
    ///
    /// - Parameters:
    ///   - ideaId: UUID string of the .idea being versioned.
    ///   - name: Human-readable version name (e.g. "v1.0", "draft-3").
    ///   - snapshotClockJSON: JSON `VectorClock` capturing the CRDT state.
    ///   - author: Public key of the tag author.
    /// - Returns: JSON string representing the created `VersionTag`.
    public static func new(
        ideaId: String,
        name: String,
        snapshotClockJSON: String,
        author: String
    ) throws -> String {
        guard let json = divi_yoke_version_tag_new(ideaId, name, snapshotClockJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create VersionTag")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a message to a version tag (like a commit message).
    ///
    /// - Returns: Modified `VersionTag` JSON.
    public static func withMessage(tagJSON: String, message: String) throws -> String {
        guard let json = divi_yoke_version_tag_with_message(tagJSON, message) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add message to VersionTag")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the branch of a version tag.
    ///
    /// - Returns: Modified `VersionTag` JSON with the new branch.
    public static func onBranch(tagJSON: String, branch: String) throws -> String {
        guard let json = divi_yoke_version_tag_on_branch(tagJSON, branch) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set branch on VersionTag")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - YokeVersionChain

/// Version chains -- the full history of an .idea across branches and merges.
///
/// Supports branching (like git), tagging versions, and merging branches back.
public enum YokeVersionChain {

    /// Create a new empty version chain for an .idea.
    ///
    /// - Parameter ideaId: UUID string of the .idea.
    /// - Returns: JSON string representing the empty `VersionChain`.
    public static func new(ideaId: String) throws -> String {
        guard let json = divi_yoke_version_chain_new(ideaId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create VersionChain")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Tag a version on a chain.
    ///
    /// - Parameters:
    ///   - chainJSON: The existing chain as JSON.
    ///   - tagJSON: The `VersionTag` to add (as JSON).
    /// - Returns: Modified `VersionChain` JSON with the new tag.
    public static func tagVersion(chainJSON: String, tagJSON: String) throws -> String {
        guard let json = divi_yoke_version_chain_tag_version(chainJSON, tagJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to tag version on VersionChain")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create a branch from a specific version.
    ///
    /// - Parameters:
    ///   - chainJSON: The existing chain as JSON.
    ///   - branchName: Name for the new branch.
    ///   - fromVersionId: UUID of the version to branch from.
    ///   - author: Public key of the branch creator.
    /// - Returns: Modified `VersionChain` JSON with the new branch.
    public static func createBranch(
        chainJSON: String,
        branchName: String,
        fromVersionId: String,
        author: String
    ) throws -> String {
        guard let json = divi_yoke_version_chain_create_branch(
            chainJSON, branchName, fromVersionId, author
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create branch on VersionChain")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Merge a source branch into a target branch.
    ///
    /// - Parameters:
    ///   - chainJSON: The existing chain as JSON.
    ///   - sourceBranch: The branch being merged.
    ///   - targetBranch: The branch receiving the merge.
    ///   - mergeVersionId: UUID for the merge version record.
    ///   - author: Public key of the merge author.
    /// - Returns: Modified `VersionChain` JSON after merge.
    public static func mergeBranch(
        chainJSON: String,
        sourceBranch: String,
        targetBranch: String,
        mergeVersionId: String,
        author: String
    ) throws -> String {
        guard let json = divi_yoke_version_chain_merge_branch(
            chainJSON, sourceBranch, targetBranch, mergeVersionId, author
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to merge branch on VersionChain")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all versions on a specific branch, chronologically.
    ///
    /// - Returns: JSON array of `VersionTag`.
    public static func versionsOnBranch(chainJSON: String, branch: String) throws -> String {
        guard let json = divi_yoke_version_chain_versions_on_branch(chainJSON, branch) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get versions on branch")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the latest version on a branch.
    ///
    /// - Returns: JSON `VersionTag`, or `nil` if the branch is empty.
    public static func latestVersion(chainJSON: String, branch: String) throws -> String? {
        guard let json = divi_yoke_version_chain_latest_version(chainJSON, branch) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Find a version by name across all branches.
    ///
    /// - Returns: JSON `VersionTag`, or `nil` if not found.
    public static func versionByName(chainJSON: String, name: String) throws -> String? {
        guard let json = divi_yoke_version_chain_version_by_name(chainJSON, name) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all branch names (including "main").
    ///
    /// - Returns: JSON array of strings.
    public static func branchNames(chainJSON: String) throws -> String {
        guard let json = divi_yoke_version_chain_branch_names(chainJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get branch names")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Check if a branch has been merged.
    public static func isBranchMerged(chainJSON: String, branchName: String) throws -> Bool {
        let result = divi_yoke_version_chain_is_branch_merged(chainJSON, branchName)
        if result < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check branch merge status")
        }
        return result == 1
    }

    /// Get the total number of versions across all branches.
    public static func versionCount(chainJSON: String) throws -> Int {
        let count = divi_yoke_version_chain_version_count(chainJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get version count")
        }
        return Int(count)
    }

    /// Get the total number of branches (including "main").
    public static func branchCount(chainJSON: String) throws -> Int {
        let count = divi_yoke_version_chain_branch_count(chainJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get branch count")
        }
        return Int(count)
    }
}

// MARK: - YokeActivity

/// Activity records -- who did what to which entity.
///
/// Actions are JSON strings like `"Created"`, `"Modified"`, or `{"Custom":"archived"}`.
/// Target types are JSON strings like `"Idea"` or `{"Custom":"project"}`.
public enum YokeActivity {

    /// Create a new activity record.
    ///
    /// - Parameters:
    ///   - actor: Public key of who performed the action.
    ///   - actionJSON: JSON `ActivityAction` (e.g. `"Created"`).
    ///   - targetId: Entity ID the action was performed on.
    ///   - targetTypeJSON: JSON `TargetType` (e.g. `"Idea"`).
    /// - Returns: JSON string representing the `ActivityRecord`.
    public static func new(
        actor: String,
        actionJSON: String,
        targetId: String,
        targetTypeJSON: String
    ) throws -> String {
        guard let json = divi_yoke_activity_record_new(actor, actionJSON, targetId, targetTypeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create ActivityRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add context to an activity record (free-form description).
    ///
    /// - Returns: Modified `ActivityRecord` JSON.
    public static func withContext(recordJSON: String, context: String) throws -> String {
        guard let json = divi_yoke_activity_record_with_context(recordJSON, context) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add context to ActivityRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the community on an activity record.
    ///
    /// - Returns: Modified `ActivityRecord` JSON.
    public static func inCommunity(recordJSON: String, communityId: String) throws -> String {
        guard let json = divi_yoke_activity_record_in_community(recordJSON, communityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set community on ActivityRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - YokeMilestone

/// Milestones -- significant moments in an entity's history.
///
/// Significance levels are JSON strings like `"Minor"`, `"Notable"`, `"Major"`,
/// `"Historic"`, or `"Epochal"`.
public enum YokeMilestone {

    /// Create a new milestone.
    ///
    /// - Parameters:
    ///   - name: Human-readable milestone name.
    ///   - significanceJSON: JSON `MilestoneSignificance` (e.g. `"Major"`).
    ///   - author: Public key of the milestone author.
    /// - Returns: JSON string representing the `Milestone`.
    public static func new(
        name: String,
        significanceJSON: String,
        author: String
    ) throws -> String {
        guard let json = divi_yoke_milestone_new(name, significanceJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create Milestone")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a description to a milestone.
    ///
    /// - Returns: Modified `Milestone` JSON.
    public static func withDescription(milestoneJSON: String, description: String) throws -> String {
        guard let json = divi_yoke_milestone_with_description(milestoneJSON, description) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add description to Milestone")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the community on a milestone.
    ///
    /// - Returns: Modified `Milestone` JSON.
    public static func inCommunity(milestoneJSON: String, communityId: String) throws -> String {
        guard let json = divi_yoke_milestone_in_community(milestoneJSON, communityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set community on Milestone")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a related event to a milestone.
    ///
    /// - Returns: Modified `Milestone` JSON.
    public static func withRelatedEvent(milestoneJSON: String, eventId: String) throws -> String {
        guard let json = divi_yoke_milestone_with_related_event(milestoneJSON, eventId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add related event to Milestone")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - YokeTimeline

/// Timelines -- ordered sequences of activities and milestones for an entity.
///
/// Supports recording events, querying by actor/action/target/community/time range,
/// pruning old entries, and filtering milestones by significance.
public enum YokeTimeline {

    /// Create a new empty timeline.
    ///
    /// - Parameter ownerId: Entity ID that owns this timeline.
    /// - Returns: JSON string representing the empty `Timeline`.
    public static func new(ownerId: String) throws -> String {
        guard let json = divi_yoke_timeline_new(ownerId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create Timeline")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the config on a timeline.
    ///
    /// - Parameters:
    ///   - timelineJSON: The existing timeline as JSON.
    ///   - configJSON: JSON `TimelineConfig`.
    /// - Returns: Modified `Timeline` JSON.
    public static func withConfig(timelineJSON: String, configJSON: String) throws -> String {
        guard let json = divi_yoke_timeline_with_config(timelineJSON, configJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set config on Timeline")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Record an activity on the timeline.
    ///
    /// - Parameters:
    ///   - timelineJSON: The existing timeline as JSON.
    ///   - activityJSON: JSON `ActivityRecord` to add.
    /// - Returns: Modified `Timeline` JSON with the activity recorded.
    public static func record(timelineJSON: String, activityJSON: String) throws -> String {
        guard let json = divi_yoke_timeline_record(timelineJSON, activityJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record activity on Timeline")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Mark a milestone on the timeline.
    ///
    /// - Parameters:
    ///   - timelineJSON: The existing timeline as JSON.
    ///   - milestoneJSON: JSON `Milestone` to mark.
    /// - Returns: Modified `Timeline` JSON with the milestone added.
    public static func markMilestone(timelineJSON: String, milestoneJSON: String) throws -> String {
        guard let json = divi_yoke_timeline_mark_milestone(timelineJSON, milestoneJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to mark milestone on Timeline")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Prune activities older than a cutoff timestamp.
    ///
    /// - Parameters:
    ///   - timelineJSON: The existing timeline as JSON.
    ///   - cutoff: Unix timestamp in seconds. Activities before this are removed.
    /// - Returns: Modified `Timeline` JSON with old activities pruned.
    public static func pruneBefore(timelineJSON: String, cutoff: Int64) throws -> String {
        guard let json = divi_yoke_timeline_prune_before(timelineJSON, cutoff) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to prune Timeline")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query activities by actor.
    ///
    /// - Returns: JSON array of `ActivityRecord`.
    public static func byActor(timelineJSON: String, actor: String) throws -> String {
        guard let json = divi_yoke_timeline_by_actor(timelineJSON, actor) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query Timeline by actor")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query activities by action type.
    ///
    /// - Parameters:
    ///   - timelineJSON: The timeline as JSON.
    ///   - actionJSON: JSON `ActivityAction` to filter by (e.g. `"Created"`).
    /// - Returns: JSON array of `ActivityRecord`.
    public static func byAction(timelineJSON: String, actionJSON: String) throws -> String {
        guard let json = divi_yoke_timeline_by_action(timelineJSON, actionJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query Timeline by action")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query activities targeting a specific entity.
    ///
    /// - Returns: JSON array of `ActivityRecord`.
    public static func forTarget(timelineJSON: String, targetId: String) throws -> String {
        guard let json = divi_yoke_timeline_for_target(timelineJSON, targetId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query Timeline for target")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query activities in a specific community.
    ///
    /// - Returns: JSON array of `ActivityRecord`.
    public static func inCommunity(timelineJSON: String, communityId: String) throws -> String {
        guard let json = divi_yoke_timeline_in_community(timelineJSON, communityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query Timeline in community")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query activities within a time range.
    ///
    /// - Parameters:
    ///   - timelineJSON: The timeline as JSON.
    ///   - since: Unix timestamp (seconds) -- start of range.
    ///   - until: Unix timestamp (seconds) -- end of range.
    /// - Returns: JSON array of `ActivityRecord`.
    public static func between(timelineJSON: String, since: Int64, until: Int64) throws -> String {
        guard let json = divi_yoke_timeline_between(timelineJSON, since, until) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query Timeline between timestamps")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Query milestones at or above a given significance level.
    ///
    /// - Parameters:
    ///   - timelineJSON: The timeline as JSON.
    ///   - significanceJSON: JSON `MilestoneSignificance` threshold (e.g. `"Major"`).
    /// - Returns: JSON array of `Milestone`.
    public static func milestonesAtLeast(
        timelineJSON: String,
        significanceJSON: String
    ) throws -> String {
        guard let json = divi_yoke_timeline_milestones_at_least(timelineJSON, significanceJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query milestones")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the total number of activities in the timeline.
    public static func activityCount(timelineJSON: String) throws -> Int {
        let count = divi_yoke_timeline_activity_count(timelineJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get activity count")
        }
        return Int(count)
    }

    /// Get the total number of milestones in the timeline.
    public static func milestoneCount(timelineJSON: String) throws -> Int {
        let count = divi_yoke_timeline_milestone_count(timelineJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get milestone count")
        }
        return Int(count)
    }
}

// MARK: - YokeCeremony

/// Ceremony records -- formal events like Covenant oaths, identity verification,
/// community founding, and rites of passage.
///
/// Ceremony types are JSON strings like `"CovenantOath"`, `"IdentityVerification"`,
/// or `{"Custom":"graduation"}`.
public enum YokeCeremony {

    /// Create a new ceremony record.
    ///
    /// - Parameter ceremonyTypeJSON: JSON `CeremonyType` (e.g. `"CovenantOath"`).
    /// - Returns: JSON string representing the `CeremonyRecord`.
    public static func new(ceremonyTypeJSON: String) throws -> String {
        guard let json = divi_yoke_ceremony_new(ceremonyTypeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a principal (primary subject) to the ceremony.
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withPrincipal(ceremonyJSON: String, crownId: String) throws -> String {
        guard let json = divi_yoke_ceremony_with_principal(ceremonyJSON, crownId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add principal to CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a witness to the ceremony.
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withWitness(ceremonyJSON: String, crownId: String) throws -> String {
        guard let json = divi_yoke_ceremony_with_witness(ceremonyJSON, crownId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add witness to CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add an officiant to the ceremony.
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withOfficiant(ceremonyJSON: String, crownId: String) throws -> String {
        guard let json = divi_yoke_ceremony_with_officiant(ceremonyJSON, crownId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add officiant to CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a participant with a custom role to the ceremony.
    ///
    /// - Parameters:
    ///   - ceremonyJSON: The existing ceremony as JSON.
    ///   - crownId: The participant's public key.
    ///   - roleJSON: JSON `ParticipantRole` (e.g. `"Witness"` or `{"Custom":"mentor"}`).
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withParticipant(
        ceremonyJSON: String,
        crownId: String,
        roleJSON: String
    ) throws -> String {
        guard let json = divi_yoke_ceremony_with_participant(ceremonyJSON, crownId, roleJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add participant to CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the community this ceremony belongs to.
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func inCommunity(ceremonyJSON: String, communityId: String) throws -> String {
        guard let json = divi_yoke_ceremony_in_community(ceremonyJSON, communityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set community on CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the content of the ceremony (oath text, charter, etc.).
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withContent(ceremonyJSON: String, content: String) throws -> String {
        guard let json = divi_yoke_ceremony_with_content(ceremonyJSON, content) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set content on CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a related event to the ceremony.
    ///
    /// - Returns: Modified `CeremonyRecord` JSON.
    public static func withRelatedEvent(ceremonyJSON: String, eventId: String) throws -> String {
        guard let json = divi_yoke_ceremony_with_related_event(ceremonyJSON, eventId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add related event to CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Validate a ceremony against its structural rules.
    ///
    /// Throws if validation fails (e.g. missing required principals).
    public static func validate(ceremonyJSON: String) throws {
        let result = divi_yoke_ceremony_validate(ceremonyJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "CeremonyRecord validation failed")
        }
    }

    /// Get all principals in the ceremony.
    ///
    /// - Returns: JSON array of public key strings.
    public static func principals(ceremonyJSON: String) throws -> String {
        guard let json = divi_yoke_ceremony_principals(ceremonyJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get principals from CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all witnesses in the ceremony.
    ///
    /// - Returns: JSON array of public key strings.
    public static func witnesses(ceremonyJSON: String) throws -> String {
        guard let json = divi_yoke_ceremony_witnesses(ceremonyJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get witnesses from CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all officiants in the ceremony.
    ///
    /// - Returns: JSON array of public key strings.
    public static func officiants(ceremonyJSON: String) throws -> String {
        guard let json = divi_yoke_ceremony_officiants(ceremonyJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get officiants from CeremonyRecord")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the total participant count of the ceremony.
    public static func participantCount(ceremonyJSON: String) throws -> Int {
        let count = divi_yoke_ceremony_participant_count(ceremonyJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get participant count")
        }
        return Int(count)
    }
}

// MARK: - YokeGraph

/// Relationship graphs -- the provenance web connecting entities through links.
///
/// All operations take and return `GraphSnapshot` JSON. The Rust side rebuilds
/// the live graph from the snapshot for each operation (stateless round-trip).
public enum YokeGraph {

    /// Create a new empty graph.
    ///
    /// - Returns: JSON string representing an empty `GraphSnapshot`.
    public static func new() -> String {
        let json = divi_yoke_graph_new()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Add a link to the graph.
    ///
    /// - Parameters:
    ///   - graphJSON: The existing graph as `GraphSnapshot` JSON.
    ///   - linkJSON: The `YokeLink` to add (as JSON).
    /// - Returns: Modified `GraphSnapshot` JSON.
    public static func addLink(graphJSON: String, linkJSON: String) throws -> String {
        guard let json = divi_yoke_graph_add_link(graphJSON, linkJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add link to graph")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Remove all links involving an entity from the graph.
    ///
    /// - Returns: Modified `GraphSnapshot` JSON.
    public static func removeEntity(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_remove_entity(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove entity from graph")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all links originating from an entity.
    ///
    /// - Returns: JSON array of `YokeLink`.
    public static func linksFrom(graphJSON: String, source: String) throws -> String {
        guard let json = divi_yoke_graph_links_from(graphJSON, source) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get links from entity")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all links pointing to an entity.
    ///
    /// - Returns: JSON array of `YokeLink`.
    public static func linksTo(graphJSON: String, target: String) throws -> String {
        guard let json = divi_yoke_graph_links_to(graphJSON, target) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get links to entity")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Find all ancestors via provenance links (DerivedFrom, VersionOf, etc.).
    ///
    /// - Returns: JSON array of `TraversalNode` (entity_id, depth, path).
    public static func ancestors(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_ancestors(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get ancestors")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Find all descendants via provenance links.
    ///
    /// - Returns: JSON array of `TraversalNode` (entity_id, depth, path).
    public static func descendants(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_descendants(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get descendants")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all comments on an entity.
    ///
    /// - Returns: JSON array of `YokeLink` with `CommentOn` relation type.
    public static func commentsOn(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_comments_on(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get comments on entity")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all version-of links pointing to an entity.
    ///
    /// - Returns: JSON array of `YokeLink` with `VersionOf` relation type.
    public static func versionsOf(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_versions_of(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get versions of entity")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all endorsements of an entity.
    ///
    /// - Returns: JSON array of `YokeLink` with `Endorses` relation type.
    public static func endorsementsOf(graphJSON: String, entityId: String) throws -> String {
        guard let json = divi_yoke_graph_endorsements_of(graphJSON, entityId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get endorsements of entity")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Find the shortest path between two entities.
    ///
    /// - Returns: JSON array of entity ID strings, or `nil` if no path exists.
    public static func pathBetween(
        graphJSON: String,
        from: String,
        to: String
    ) throws -> String? {
        guard let json = divi_yoke_graph_path_between(graphJSON, from, to) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the total number of links in the graph.
    public static func linkCount(graphJSON: String) throws -> Int {
        let count = divi_yoke_graph_link_count(graphJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get link count")
        }
        return Int(count)
    }

    /// Get the total number of unique entities in the graph.
    public static func entityCount(graphJSON: String) throws -> Int {
        let count = divi_yoke_graph_entity_count(graphJSON)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get entity count")
        }
        return Int(count)
    }
}
