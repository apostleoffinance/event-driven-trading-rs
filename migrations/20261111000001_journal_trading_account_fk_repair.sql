-- Repair the optional Journal-to-trading account link without dropping data.
DO $migration$
DECLARE
    actual_type TEXT;
    actual_not_null BOOLEAN;
    source_attnum SMALLINT;
    target_attnum SMALLINT;
    constraint_row RECORD;
    constraint_name TEXT;
    constraint_definition TEXT;
    correct_constraint_found BOOLEAN := FALSE;
    orphan_count BIGINT;
BEGIN
    SELECT format_type(a.atttypid, a.atttypmod), a.attnotnull, a.attnum
      INTO actual_type, actual_not_null, source_attnum
      FROM pg_attribute a
      JOIN pg_class c ON c.oid = a.attrelid
      JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = current_schema() AND c.relname = 'journal_accounts'
       AND a.attname = 'linked_trading_account_id' AND NOT a.attisdropped;
    IF actual_type IS NULL OR actual_type <> 'text' THEN
        RAISE EXCEPTION 'journal_accounts.linked_trading_account_id must exist as TEXT; found %', actual_type;
    END IF;
    IF actual_not_null THEN
        RAISE EXCEPTION 'journal_accounts.linked_trading_account_id must remain nullable';
    END IF;

    SELECT a.attnum INTO target_attnum
      FROM pg_attribute a
      JOIN pg_class c ON c.oid = a.attrelid
      JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = current_schema() AND c.relname = 'accounts'
       AND a.attname = 'id' AND NOT a.attisdropped
       AND format_type(a.atttypid, a.atttypmod) = 'text';
    IF target_attnum IS NULL THEN
        RAISE EXCEPTION 'accounts.id must exist as TEXT';
    END IF;

    -- Recognize equivalent constraints regardless of their historical names;
    -- reject every conflicting FK touching the optional link column.
    FOR constraint_row IN
        SELECT c.conname, pg_get_constraintdef(c.oid) AS definition, c.confrelid,
               c.confupdtype, c.confdeltype, c.conkey, c.confkey, c.confmatchtype
          FROM pg_constraint c
         WHERE c.conrelid = 'journal_accounts'::regclass AND c.contype = 'f'
           AND (c.conname = 'fk_journal_accounts_trading_account'
                OR c.conkey @> ARRAY[source_attnum]::SMALLINT[])
    LOOP
        IF constraint_row.conkey = ARRAY[source_attnum]::SMALLINT[]
           AND constraint_row.confrelid = 'accounts'::regclass
           AND constraint_row.confkey = ARRAY[target_attnum]::SMALLINT[]
           AND constraint_row.confupdtype = 'a'
           AND constraint_row.confdeltype = 'a'
           AND constraint_row.confmatchtype = 's' THEN
            IF correct_constraint_found THEN
                RAISE EXCEPTION 'multiple foreign keys already constrain journal_accounts.linked_trading_account_id';
            END IF;
            correct_constraint_found := TRUE;
            constraint_name := constraint_row.conname;
            constraint_definition := constraint_row.definition;
        ELSE
            RAISE EXCEPTION 'conflicting foreign key % on journal_accounts.linked_trading_account_id: %',
                constraint_row.conname, constraint_row.definition;
        END IF;
    END LOOP;

    IF NOT correct_constraint_found THEN
        SELECT count(*) INTO orphan_count
          FROM journal_accounts j LEFT JOIN accounts a ON a.id=j.linked_trading_account_id
         WHERE j.linked_trading_account_id IS NOT NULL AND a.id IS NULL;
        IF orphan_count > 0 THEN
            RAISE EXCEPTION 'cannot add Journal trading account FK: % orphaned reference(s)', orphan_count;
        END IF;
        ALTER TABLE journal_accounts ADD CONSTRAINT fk_journal_accounts_trading_account
            FOREIGN KEY (linked_trading_account_id) REFERENCES accounts(id)
            ON UPDATE NO ACTION ON DELETE NO ACTION NOT VALID;
        constraint_name := 'fk_journal_accounts_trading_account';
    END IF;

    EXECUTE format('ALTER TABLE journal_accounts VALIDATE CONSTRAINT %I', constraint_name);
    SELECT pg_get_constraintdef(c.oid) INTO constraint_definition
      FROM pg_constraint c
     WHERE c.conrelid='journal_accounts'::regclass AND c.conname=constraint_name
       AND c.contype='f' AND c.confrelid='accounts'::regclass
       AND c.confupdtype='a' AND c.confdeltype='a' AND c.confmatchtype='s'
       AND c.conkey=ARRAY[source_attnum]::SMALLINT[]
       AND c.confkey=ARRAY[target_attnum]::SMALLINT[] AND c.convalidated;
    IF constraint_definition IS NULL THEN
        RAISE EXCEPTION 'Journal trading account FK validation failed for constraint %', constraint_name;
    END IF;
END
$migration$;
