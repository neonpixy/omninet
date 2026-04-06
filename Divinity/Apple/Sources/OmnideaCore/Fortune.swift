import COmnideaFFI
import Foundation

// MARK: - Ledger (stateful)

/// Fortune ledger — tracks all Cool balances and transactions.
public final class FortuneLedger: @unchecked Sendable {
    let ptr: OpaquePointer

    public init() {
        ptr = divi_fortune_ledger_new()!
    }

    deinit {
        divi_fortune_ledger_free(ptr)
    }

    /// Get balance for a pubkey. Returns JSON Balance.
    public func balance(pubkey: String) -> String {
        let json = divi_fortune_ledger_balance(ptr, pubkey)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Credit a pubkey with Cool.
    /// `reason` is JSON like `"Ubi"` or `"Transfer"`.
    public func credit(pubkey: String, amount: Int64, reason: String, reference: String? = nil) throws {
        let result = divi_fortune_ledger_credit(ptr, pubkey, amount, reason, reference)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Credit failed")
        }
    }

    /// Debit Cool from a pubkey.
    public func debit(pubkey: String, amount: Int64, reason: String, reference: String? = nil) throws {
        let result = divi_fortune_ledger_debit(ptr, pubkey, amount, reason, reference)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Debit failed")
        }
    }

    /// Transfer Cool between two pubkeys.
    public func transfer(sender: String, recipient: String, amount: Int64, reference: String? = nil) throws {
        let result = divi_fortune_ledger_transfer(ptr, sender, recipient, amount, reference)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Transfer failed")
        }
    }

    /// Get transaction history for a pubkey. Returns JSON array of Transactions.
    public func transactions(pubkey: String) -> String {
        let json = divi_fortune_ledger_transactions(ptr, pubkey)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get a transaction summary for a pubkey. Returns JSON TransactionSummary.
    public func summary(pubkey: String) -> String {
        let json = divi_fortune_ledger_summary(ptr, pubkey)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Treasury (stateful)

/// Fortune treasury — tracks Cool supply backed by network capacity.
public final class FortuneTreasury: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a treasury with a policy. Pass nil for default policy.
    public init(policyJSON: String? = nil) {
        ptr = divi_fortune_treasury_new(policyJSON)!
    }

    deinit {
        divi_fortune_treasury_free(ptr)
    }

    /// Get treasury status. Returns JSON TreasuryStatus.
    public func status() -> String {
        let json = divi_fortune_treasury_status(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Mint Cool for a recipient. `reason` is JSON like `"Ubi"`.
    /// Returns the actual amount minted (may be capped by supply).
    public func mint(amount: Int64, recipient: String, reason: String) throws -> Int64 {
        let minted = divi_fortune_treasury_mint(ptr, amount, recipient, reason)
        if minted < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Mint failed")
        }
        return minted
    }

    /// Update network metrics (affects max supply).
    public func updateMetrics(_ metricsJSON: String) throws {
        let result = divi_fortune_treasury_update_metrics(ptr, metricsJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update metrics")
        }
    }
}

// MARK: - UBI Distributor (stateful)

/// Fortune UBI distributor — universal basic income for verified identities.
public final class FortuneUBI: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_fortune_ubi_new()!
    }

    deinit {
        divi_fortune_ubi_free(ptr)
    }

    /// Mark a pubkey as verified for UBI eligibility.
    public func verifyIdentity(_ pubkey: String) {
        divi_fortune_ubi_verify_identity(ptr, pubkey)
    }

    /// Check UBI eligibility. Returns JSON UbiEligibility.
    public func checkEligibility(
        pubkey: String, ledger: FortuneLedger, treasury: FortuneTreasury, policyJSON: String? = nil
    ) throws -> String {
        guard let json = divi_fortune_ubi_check_eligibility(
            ptr, pubkey, ledger.ptr, treasury.ptr, policyJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check eligibility")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Claim UBI. Returns JSON ClaimRecord on success.
    public func claim(
        pubkey: String, ledger: FortuneLedger, treasury: FortuneTreasury, policyJSON: String? = nil
    ) throws -> String {
        guard let json = divi_fortune_ubi_claim(
            ptr, pubkey, ledger.ptr, treasury.ptr, policyJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "UBI claim failed")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Demurrage

public enum FortuneDemurrage {

    /// Preview demurrage for a pubkey. Returns JSON DemurragePreview.
    public static func preview(pubkey: String, ledger: FortuneLedger, policyJSON: String? = nil) throws -> String {
        guard let json = divi_fortune_demurrage_preview(pubkey, ledger.ptr, policyJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to preview demurrage")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Policy

public enum FortuneConfiguration {

    /// Get the default FortunePolicy as JSON.
    public static func defaultPolicy() -> String {
        let json = divi_fortune_policy_default()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Trade

public enum FortuneTrades {

    /// Create a trade proposal. Returns JSON TradeProposal.
    public static func create(
        proposer: String, recipient: String, offeringCool: Int64, requestingCool: Int64
    ) throws -> String {
        guard let json = divi_fortune_trade_new(proposer, recipient, offeringCool, requestingCool) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create trade")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Accept a trade proposal. Returns updated trade JSON.
    public static func accept(tradeJSON: String) throws -> String {
        guard let json = divi_fortune_trade_accept(tradeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to accept trade")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Execute a trade proposal. Returns updated trade JSON.
    public static func execute(tradeJSON: String) throws -> String {
        guard let json = divi_fortune_trade_execute(tradeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to execute trade")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Cash

public enum FortuneCash {

    /// Generate a random cash serial number.
    public static func generateSerial() -> String {
        let serial = divi_fortune_cash_generate_serial()!
        defer { divi_free_string(serial) }
        return String(cString: serial)
    }

    /// Validate a cash serial number format.
    public static func validateSerial(_ serial: String) -> Bool {
        divi_fortune_cash_validate_serial(serial)
    }
}
