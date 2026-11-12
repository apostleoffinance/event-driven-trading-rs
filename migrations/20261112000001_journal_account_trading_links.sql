-- A trading account may be linked to at most one active Journal account per owner.
-- Access remains governed by dashboard_account_access; this index does not grant access.
CREATE UNIQUE INDEX IF NOT EXISTS uq_journal_accounts_owner_trading_link
ON journal_accounts(owner_id, linked_trading_account_id)
WHERE linked_trading_account_id IS NOT NULL AND archived=FALSE;
