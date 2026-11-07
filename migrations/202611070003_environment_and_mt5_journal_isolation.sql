-- Environment isolation: environment is an account-owned dimension, never inferred
-- from UI mode. Journal records inherit their environment from journal_accounts.
-- Account environment supports LIVE, DEMO, PAPER (and existing SHADOW mode).
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_environment_check;
ALTER TABLE accounts
    ADD CONSTRAINT accounts_environment_check
    CHECK (environment IN ('PAPER', 'SHADOW', 'DEMO', 'LIVE'));

ALTER TABLE journal_accounts
    ADD COLUMN IF NOT EXISTS environment TEXT NOT NULL DEFAULT 'PAPER'
        CHECK (environment IN ('PAPER', 'DEMO', 'LIVE'));

-- Preserve historical account classifications where they are explicit.
UPDATE journal_accounts
SET environment = CASE
    WHEN account_type = 'live' THEN 'LIVE'
    WHEN account_type = 'demo' THEN 'DEMO'
    WHEN account_type IN ('paper', 'funded') THEN 'PAPER'
    ELSE 'PAPER'
END
WHERE environment = 'PAPER';

CREATE INDEX IF NOT EXISTS idx_journal_accounts_owner_environment_active
    ON journal_accounts(owner_id, environment, archived);

COMMENT ON COLUMN accounts.environment IS
    'Trading environment boundary: PAPER, DEMO, SHADOW, or LIVE.';

COMMENT ON COLUMN journal_accounts.environment IS
    'Journal account environment boundary: LIVE, DEMO, or PAPER. All journal trades inherit this value.';

-- MT5 imports are broker-authoritative. Annotation edits remain permitted, but
-- core execution facts cannot be rewritten by clients through Journal PUT APIs.
CREATE OR REPLACE FUNCTION edgenest_guard_mt5_journal_trade_update()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.source = 'mt5' THEN
        IF NEW.source IS DISTINCT FROM OLD.source
           OR NEW.source_system IS DISTINCT FROM OLD.source_system
           OR NEW.source_trade_id IS DISTINCT FROM OLD.source_trade_id
           OR NEW.journal_account_id IS DISTINCT FROM OLD.journal_account_id
           OR NEW.symbol IS DISTINCT FROM OLD.symbol
           OR NEW.direction IS DISTINCT FROM OLD.direction
           OR NEW.status IS DISTINCT FROM OLD.status
           OR NEW.opened_at IS DISTINCT FROM OLD.opened_at
           OR NEW.closed_at IS DISTINCT FROM OLD.closed_at
           OR NEW.quantity IS DISTINCT FROM OLD.quantity
           OR NEW.entry_price IS DISTINCT FROM OLD.entry_price
           OR NEW.exit_price IS DISTINCT FROM OLD.exit_price
           OR NEW.stop_loss IS DISTINCT FROM OLD.stop_loss
           OR NEW.take_profit IS DISTINCT FROM OLD.take_profit
           OR NEW.gross_pnl IS DISTINCT FROM OLD.gross_pnl
           OR NEW.commission IS DISTINCT FROM OLD.commission
           OR NEW.swap IS DISTINCT FROM OLD.swap
           OR NEW.other_fees IS DISTINCT FROM OLD.other_fees
           OR NEW.net_pnl IS DISTINCT FROM OLD.net_pnl
           OR NEW.pnl_currency IS DISTINCT FROM OLD.pnl_currency
           OR NEW.source_deal_ids IS DISTINCT FROM OLD.source_deal_ids
           OR NEW.source_payload IS DISTINCT FROM OLD.source_payload
        THEN
            RAISE EXCEPTION 'MT5 execution facts are broker-authoritative and immutable'
                USING ERRCODE = 'check_violation';
        END IF;
    END IF;

    IF NEW.source = 'mt5' THEN
        IF NOT EXISTS (
            SELECT 1 FROM journal_accounts ja
            WHERE ja.owner_id = NEW.owner_id
              AND ja.id = NEW.journal_account_id
              AND ja.environment IN ('DEMO', 'LIVE')
              AND ja.archived = FALSE
        ) THEN
            RAISE EXCEPTION 'MT5-sourced journal trades require an active LIVE or DEMO account'
                USING ERRCODE = 'check_violation';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_guard_mt5_journal_trade_update ON journal_trades;
CREATE TRIGGER trg_guard_mt5_journal_trade_update
BEFORE UPDATE ON journal_trades
FOR EACH ROW EXECUTE FUNCTION edgenest_guard_mt5_journal_trade_update();

-- Prevent the environment of an account from being changed after it has
-- accumulated journal records. Moving data between environments requires an
-- explicit migration, never an ordinary account edit.
CREATE OR REPLACE FUNCTION edgenest_guard_journal_account_environment()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.environment IS DISTINCT FROM OLD.environment
       AND EXISTS (
           SELECT 1 FROM journal_trades jt
           WHERE jt.owner_id = OLD.owner_id
             AND jt.journal_account_id = OLD.id
       )
    THEN
        RAISE EXCEPTION 'Cannot change journal account environment after trades exist'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_guard_journal_account_environment ON journal_accounts;
CREATE TRIGGER trg_guard_journal_account_environment
BEFORE UPDATE ON journal_accounts
FOR EACH ROW EXECUTE FUNCTION edgenest_guard_journal_account_environment();
