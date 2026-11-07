#!/usr/bin/env python3
"""TraderOS -> EdgeNest Journal migration utility.

Read-only source extraction. EdgeNest API is the only write path. No auth/session/MT5
credential tables are queried. Requires psycopg[binary] and requests.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

SOURCE_TABLES = (
    "accounts", "account_risk_profiles", "setups", "trades", "psychology",
    "checklist_templates", "checklist_items", "trade_checklist_responses",
    "trade_screenshots",
)
SECRETISH = re.compile(r"(password|token|secret|credential|private.?key|refresh.?token)", re.I)


def json_value(value: Any) -> Any:
    if isinstance(value, (datetime, date)):
        return value.isoformat()
    if isinstance(value, Decimal):
        return str(value)
    if isinstance(value, bytes):
        return {"__binary_sha256__": hashlib.sha256(value).hexdigest(), "__binary_size__": len(value)}
    if isinstance(value, dict):
        return {str(k): json_value(v) for k, v in value.items() if not SECRETISH.search(str(k))}
    if isinstance(value, (list, tuple)):
        return [json_value(v) for v in value]
    return value


def dump_json(path: Path, obj: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(json_value(obj), indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def connect_source(dsn: str):
    try:
        import psycopg
        from psycopg.rows import dict_row
    except ImportError as exc:
        raise SystemExit("Missing psycopg. Install scripts/traderos-migration/requirements.txt") from exc
    conn = psycopg.connect(dsn, row_factory=dict_row, connect_timeout=15)
    conn.execute("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
    conn.execute("SET statement_timeout = '120s'")
    return conn


def source_snapshot(dsn: str, legacy_user_id: str, out: Path) -> dict[str, Any]:
    """Export only the selected user's journal rows and related safe records."""
    conn = connect_source(dsn)
    files_dir = out.with_suffix("").with_name(out.stem + "_screenshot_files")
    counts: dict[str, int] = {}
    try:
        with conn.transaction():
            for table in SOURCE_TABLES:
                if table == "account_risk_profiles":
                    sql = """SELECT arp.* FROM account_risk_profiles arp
                             JOIN accounts a ON a.id=arp.account_id WHERE a.user_id=%s"""
                elif table == "checklist_items":
                    sql = """SELECT ci.* FROM checklist_items ci JOIN checklist_templates ct
                             ON ct.id=ci.template_id WHERE ct.user_id=%s"""
                elif table == "trade_checklist_responses":
                    sql = """SELECT r.* FROM trade_checklist_responses r JOIN trades t
                             ON t.id=r.trade_id WHERE t.user_id=%s"""
                elif table == "trade_screenshots":
                    sql = """SELECT s.* FROM trade_screenshots s JOIN trades t
                             ON t.id=s.trade_id WHERE s.user_id=%s AND t.user_id=%s"""
                elif table == "psychology":
                    sql = """SELECT p.* FROM psychology p JOIN trades t ON t.id=p.trade_id
                             WHERE p.user_id=%s AND t.user_id=%s"""
                else:
                    sql = f"SELECT * FROM {table} WHERE user_id=%s"
                args = (legacy_user_id, legacy_user_id) if table in ("trade_screenshots", "psychology") else (legacy_user_id,)
                rows = conn.execute(sql, args).fetchall()
                clean_rows = []
                for row in rows:
                    item = dict(row)
                    if table == "trade_screenshots":
                        binary = item.pop("file_data", None)
                        key = str(item.get("id", "unknown"))
                        if binary:
                            files_dir.mkdir(parents=True, exist_ok=True)
                            target = files_dir / f"{key}.bin"
                            target.write_bytes(binary)
                            item["file_path"] = str(target)
                            item["file_sha256"] = hashlib.sha256(binary).hexdigest()
                            item["file_size"] = len(binary)
                    clean_rows.append(json_value(item))
                dump_json(out.with_suffix("").with_name(f"{out.stem}_{table}.json"), clean_rows)
                counts[table] = len(clean_rows)
    finally:
        conn.close()
    manifest = {
        "format": "traderos-edgenest-migration-v1",
        "created_at": datetime.now().astimezone().isoformat(),
        "source_user_id": str(legacy_user_id),
        "tables": counts,
        "screenshot_files_dir": str(files_dir) if files_dir.exists() else None,
        "snapshot_prefix": out.stem,
    }
    dump_json(out, manifest)
    return manifest


def parse_time(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, datetime):
        dt = value
    else:
        dt = datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    if dt.tzinfo is None:
        raise ValueError(f"Naive timestamp encountered: {value!r}; source timezone cannot be inferred")
    return dt.isoformat()


def decimal_str(value: Any, default: str | None = None) -> str | None:
    if value is None:
        return default
    return str(Decimal(str(value)))


def api_request(base: str, token: str, method: str, path: str, body: Any = None, retries: int = 5) -> Any:
    try:
        import requests
    except ImportError as exc:
        raise SystemExit("Missing requests. Install scripts/traderos-migration/requirements.txt") from exc
    url = base.rstrip("/") + path
    for attempt in range(retries):
        response = requests.request(method, url, headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
                                    json=body, timeout=(10, 90))
        if response.status_code in (429, 502, 503, 504) and attempt < retries - 1:
            time.sleep(min(2 ** attempt, 20))
            continue
        if response.status_code >= 400:
            raise RuntimeError(f"{method} {path} -> {response.status_code}: {response.text[:1200]}")
        if response.status_code == 204 or not response.content:
            return None
        return response.json()
    raise RuntimeError(f"Retry exhaustion for {method} {path}")


def read_table(prefix: Path, table: str) -> list[dict[str, Any]]:
    p = prefix.with_suffix("").with_name(f"{prefix.stem}_{table}.json")
    return load_json(p) if p.exists() else []


def make_trade(row: dict[str, Any], account_id: str, setup_map: dict[str, str], legacy_user_id: str, pnl_currency: str = "USD") -> dict[str, Any]:
    legacy_id = str(row["id"])
    opened = parse_time(row.get("trade_timestamp"))
    closed = parse_time(row.get("exit_timestamp"))
    status_src = str(row.get("status") or row.get("result") or "open").lower()
    closed_trade = status_src in {"closed", "complete", "completed"} or closed is not None
    status = "closed" if closed_trade else "open"
    if status == "closed" and (not closed or row.get("exit_price") is None):
        raise ValueError(f"Closed trade {legacy_id} has missing exit timestamp or exit price")
    direction = str(row.get("direction") or "long").lower()
    if direction not in ("long", "short"):
        raise ValueError(f"Unsupported trade direction {direction!r} for trade {legacy_id}")
    setup_id = setup_map.get(str(row.get("setup_id"))) if row.get("setup_id") else None
    # source system + stable source ID makes imports idempotent.
    entry = {
        "journal_account_id": account_id,
        "setup_id": setup_id,
        "symbol": str(row.get("symbol") or row.get("symbol_raw") or "").upper(),
        "direction": direction,
        "status": status,
        "source": "legacy_import",
        "source_system": "TraderOS",
        "source_trade_id": legacy_id,
        "opened_at": opened,
        "closed_at": closed if status == "closed" else None,
        "quantity": decimal_str(row.get("lot_size"), "0"),
        "entry_price": decimal_str(row.get("entry_price"), "0"),
        "exit_price": decimal_str(row.get("exit_price")),
        "stop_loss": decimal_str(row.get("stop_loss")),
        "take_profit": decimal_str(row.get("take_profit")),
        "gross_pnl": (str(Decimal(str(row.get("realized_pnl"))) - Decimal(str(row.get("commission") or 0)) - Decimal(str(row.get("swap") or 0))) if row.get("realized_pnl") is not None else None),
        "commission": decimal_str(row.get("commission"), "0"),
        "swap": decimal_str(row.get("swap"), "0"),
        "other_fees": "0",
        "net_pnl": decimal_str(row.get("realized_pnl")),
        "pnl_currency": str(pnl_currency or "USD").upper(),
        "risk_amount": decimal_str(row.get("risk_amount")),
        "r_multiple": decimal_str(row.get("realized_r")),
        "session": row.get("session"),
        "timeframe": row.get("timeframe"),
        "timezone": row.get("timezone"),
        "mfe_r": decimal_str(row.get("mfe_r")),
        "mae_r": decimal_str(row.get("mae_r")),
        "notes": row.get("notes"),
        "tags": [],
        "setup_valid": row.get("setup_valid"),
        "rules_followed": row.get("rules_followed"),
        "emotional_trade": row.get("emotional_trade"),
        "mistake": row.get("mistake"),
        "mistake_notes": row.get("mistake_notes"),
        "discipline_score": row.get("discipline_score"),
        "acknowledged_warnings": ["TraderOS acknowledged warnings"] if row.get("acknowledged_warnings") else [],
    }
    # retain source-only values, source external IDs and precision in lineage payload.
    entry["_migration"] = {
        "legacy_user_id": legacy_user_id,
        "legacy_trade_id": legacy_id,
        "external_provider": row.get("external_provider"),
        "external_position_id": row.get("external_position_id"),
        "external_deal_id": row.get("external_deal_id"),
        "symbol_raw": row.get("symbol_raw"),
        "instrument_status": row.get("instrument_status"),
        "risk_percent": decimal_str(row.get("risk_percent")),
        "planned_reward": decimal_str(row.get("planned_reward")),
        "planned_rr": decimal_str(row.get("planned_rr")),
        "realized_rr": decimal_str(row.get("realized_rr")),
        "stop_pips": decimal_str(row.get("stop_pips")),
        "tp_pips": decimal_str(row.get("tp_pips")),
        "holding_time_seconds": row.get("holding_time_seconds"),
        "source_analysis_id": row.get("source_analysis_id"),
        "created_at": row.get("created_at"),
        "updated_at": row.get("updated_at"),
    }
    return entry


def validate_snapshot(prefix: Path, legacy_user_id: str, account_map: dict[str, str], setup_map: dict[str, str]) -> dict[str, Any]:
    trades = read_table(prefix, "trades")
    accounts = read_table(prefix, "accounts")
    setups = read_table(prefix, "setups")
    errors: list[str] = []
    if not accounts:
        errors.append("No source accounts found for selected TraderOS user")
    if not trades:
        errors.append("No source trades found for selected TraderOS user")
    source_account_ids = {str(a["id"]) for a in accounts}
    for t in trades:
        if str(t.get("account_id")) not in source_account_ids:
            errors.append(f"Trade {t.get('id')} references missing source account {t.get('account_id')}")
        if str(t.get("account_id")) not in account_map:
            errors.append(f"Missing target account mapping for source account {t.get('account_id')}")
        try:
            make_trade(t, account_map.get(str(t.get("account_id")), ""), setup_map, legacy_user_id)
        except Exception as exc:
            errors.append(str(exc))
    for s in setups:
        if str(s["id"]) not in setup_map:
            errors.append(f"Missing target setup mapping for setup {s['id']}")
    return {"legacy_user_id": legacy_user_id, "accounts": len(accounts), "setups": len(setups), "trades": len(trades),
            "errors": errors, "valid": not errors}


def migrate(prefix: Path, config: dict[str, Any], dry_run: bool) -> dict[str, Any]:
    base = config["edgenest_api_url"]
    owner_map = config["owner_map"]
    report: dict[str, Any] = {"mode": "dry-run" if dry_run else "apply", "users": [], "errors": [], "counts": {}}
    state_path = Path(config.get("state_file", "migration-state.json"))
    state = load_json(state_path) if state_path.exists() else {"account_map": {}, "setup_map": {}, "trade_map": {}, "screenshot_map": {}}
    for legacy_user_id, user_cfg in owner_map.items():
        token = user_cfg.get("jwt")
        if not token:
            report["errors"].append(f"owner_map[{legacy_user_id}] is missing EdgeNest JWT")
            continue
        accounts = read_table(prefix, "accounts")
        accounts = [a for a in accounts if str(a.get("user_id")) == str(legacy_user_id)]
        setups = [s for s in read_table(prefix, "setups") if str(s.get("user_id")) == str(legacy_user_id)]
        trades = [t for t in read_table(prefix, "trades") if str(t.get("user_id")) == str(legacy_user_id)]
        acc_map = state["account_map"].setdefault(str(legacy_user_id), {})
        setup_map = state["setup_map"].setdefault(str(legacy_user_id), {})
        created_acc = created_setup = imported = skipped = psych = attachments = 0

        risk_profiles = {str(r.get("account_id")): r for r in read_table(prefix, "account_risk_profiles")}
        for account in accounts:
            legacy_acc_id = str(account["id"])
            if legacy_acc_id in acc_map:
                continue
            if dry_run:
                acc_map[legacy_acc_id] = f"DRYRUN:{legacy_acc_id}"
                continue
            acct_type = "funded" if account.get("firm") else "journal"
            profile = risk_profiles.get(legacy_acc_id, {})
            payload = {"name": str(account.get("account_name") or "TraderOS account"), "broker": " / ".join(x for x in [str(account.get("firm") or "").strip(), str(account.get("program") or "").strip()] if x) or None,
                       "account_type": acct_type, "currency": str(account.get("currency") or "USD").upper(),
                       "initial_balance": decimal_str(account.get("starting_balance"), "0"),
                       "external_source": "TraderOS", "external_account_id": legacy_acc_id,
                       "migration_metadata": {"legacy_account_id": legacy_acc_id, "legacy_firm": account.get("firm"),
                                              "legacy_program": account.get("program"),
                                              "legacy_status": account.get("status"),
                                              "legacy_current_balance": decimal_str(account.get("current_balance")),
                                              "legacy_current_equity": decimal_str(account.get("current_equity")),
                                              "legacy_risk_profile": profile}}
            result = api_request(base, token, "POST", "/api/v1/journal/accounts", payload)
            acc_map[legacy_acc_id] = result["data"]["id"]
            created_acc += 1
            dump_json(state_path, state)

        for setup in setups:
            legacy_setup_id = str(setup["id"])
            if legacy_setup_id in setup_map:
                continue
            if dry_run:
                setup_map[legacy_setup_id] = f"DRYRUN:{legacy_setup_id}"
                continue
            payload = {"name": str(setup.get("name") or f"Imported setup {legacy_setup_id}"),
                       "description": setup.get("description"),
                       "rules_json": {"traderos_setup_id": legacy_setup_id, "active": setup.get("active", True)}}
            try:
                result = api_request(base, token, "POST", "/api/v1/journal/setups", payload)
                setup_map[legacy_setup_id] = result["data"]["id"]
            except RuntimeError:
                # Existing same-name setup: resolve by name; do not silently invent a mapping.
                existing = api_request(base, token, "GET", "/api/v1/journal/setups")
                found = next((s for s in existing.get("data", []) if s.get("name") == payload["name"]), None)
                if not found:
                    raise
                setup_map[legacy_setup_id] = found["id"]
            created_setup += 1
            dump_json(state_path, state)

        validation = validate_snapshot(prefix, legacy_user_id, acc_map, setup_map)
        user_result = {"legacy_user_id": legacy_user_id, "account_count": len(accounts), "setup_count": len(setups),
                       "trade_count": len(trades), "validation": validation, "created_accounts": created_acc,
                       "created_setups": created_setup}
        report["users"].append(user_result)
        if not validation["valid"]:
            report["errors"].extend(validation["errors"])
            continue
        if dry_run:
            continue

        # Import <=10k per batch. API transaction is atomic per batch and source IDs are idempotent.
        trades_by_account: dict[str, list[dict[str, Any]]] = {}
        for trade_row in trades:
            trades_by_account.setdefault(str(trade_row["account_id"]), []).append(trade_row)
        checklist_items = {str(item["id"]): item for item in read_table(prefix, "checklist_items")}
        checklist_templates = {str(template["id"]): template for template in read_table(prefix, "checklist_templates")}
        checklist_responses = [r for r in read_table(prefix, "trade_checklist_responses")
                               if str(r.get("trade_id")) in {str(t["id"]) for t in trades}]
        checklist_by_trade: dict[str, list[dict[str, Any]]] = {}
        for response in checklist_responses:
            item = checklist_items.get(str(response.get("item_id")), {})
            template = checklist_templates.get(str(item.get("template_id")), {})
            checklist_by_trade.setdefault(str(response.get("trade_id")), []).append({
                "legacy_item_id": str(response.get("item_id")),
                "label": item.get("label"), "description": item.get("description"),
                "category": item.get("category"), "kind": item.get("kind"),
                "auto_key": item.get("auto_key"), "required": item.get("required"),
                "sort_order": item.get("sort_order"), "template_name": template.get("name"),
                "template_id": str(item.get("template_id")) if item.get("template_id") else None,
                "checked": bool(response.get("checked")),
            })
        for source_account_id, account_trades in trades_by_account.items():
            for start in range(0, len(account_trades), 10000):
                batch = account_trades[start:start + 10000]
                body_trades = []
                source_account = next(a for a in accounts if str(a["id"]) == source_account_id)
                currency = str(source_account.get("currency") or "USD")
                for trade_row in batch:
                    body = make_trade(trade_row, acc_map[source_account_id], setup_map, legacy_user_id, currency)
                    lineage = body.pop("_migration", {})
                    lineage.update({"legacy_account_id": source_account_id,
                                    "legacy_setup_id": str(trade_row.get("setup_id")) if trade_row.get("setup_id") else None})
                    body["source_payload"] = lineage
                    body["checklist"] = {"legacy_traderos_responses": checklist_by_trade.get(str(trade_row["id"]), [])}
                    body_trades.append(body)
                result = api_request(base, token, "POST", "/api/v1/journal/import",
                                     {"journal_account_id": acc_map[source_account_id],
                                      "source_system": "TraderOS", "trades": body_trades})
                imported += int(result["data"].get("imported_count", 0))
                skipped += int(result["data"].get("skipped_count", 0))
        dump_json(state_path, state)

        # Source psych entries are mapped to per-trade psychology endpoints.
        psych_rows = {str(p["trade_id"]): p for p in read_table(prefix, "psychology") if str(p.get("user_id")) == str(legacy_user_id)}
        # Resolve target trade IDs by idempotent source identity.
        target_trades_all = []
        for source_account_id, target_account_id in acc_map.items():
            offset = 0
            while True:
                page = api_request(base, token, "GET",
                                   f"/api/v1/journal/trades?journal_account_id={target_account_id}&limit=500&offset={offset}")
                rows = page.get("data", [])
                target_trades_all.extend(rows)
                if len(rows) < 500:
                    break
                offset += 500
        target_by_source = {str(t.get("source_trade_id")): t for t in target_trades_all if t.get("source_system") == "TraderOS"}
        for legacy_trade_id, p in psych_rows.items():
            target = target_by_source.get(legacy_trade_id)
            if not target:
                report["errors"].append(f"Psychology trade not found after import: {legacy_trade_id}")
                continue
            payload = {"emotion_before": p.get("emotion_before"), "emotion_during": p.get("emotion_during"),
                       "emotion_after": p.get("emotion_after"), "confidence": p.get("confidence"),
                       "fear": p.get("fear"), "fomo": p.get("fomo"), "frustration": p.get("frustration"),
                       "revenge_intensity": p.get("revenge"), "boredom": p.get("boredom"), "notes": p.get("notes")}
            if not dry_run:
                api_request(base, token, "PUT", f"/api/v1/journal/trades/{target['id']}/psychology", payload)
                psych += 1

        # Screenshot files are migrated to an S3-compatible destination when configured.
        # External URLs/storage metadata are otherwise retained in the audit report only.
        screenshot_rows = [s for s in read_table(prefix, "trade_screenshots") if str(s.get("user_id")) == str(legacy_user_id)]
        if screenshot_rows:
            s3_cfg = config.get("object_storage", {})
            if not (s3_cfg.get("bucket") and s3_cfg.get("access_key_id") and s3_cfg.get("secret_access_key")):
                report["errors"].append(f"{len(screenshot_rows)} screenshot records need object_storage bucket/access_key_id/secret_access_key configuration; bytes remain in the snapshot directory.")
            else:
                try:
                    import boto3
                    s3 = boto3.client("s3", endpoint_url=s3_cfg.get("endpoint_url"),
                                      region_name=s3_cfg.get("region", "auto"),
                                      aws_access_key_id=s3_cfg["access_key_id"],
                                      aws_secret_access_key=s3_cfg["secret_access_key"])
                    for shot in screenshot_rows:
                        old_trade_id = str(shot.get("trade_id"))
                        target_trade = target_by_source.get(old_trade_id)
                        if not target_trade:
                            report["errors"].append(f"Screenshot orphan: source trade {old_trade_id} missing in EdgeNest")
                            continue
                        source_file = shot.get("file_path")
                        if not source_file or not Path(source_file).is_file():
                            report["errors"].append(f"Screenshot {shot.get('id')} has no extracted bytes")
                            continue
                        key = f"{s3_cfg.get('prefix', 'traderos-import').strip('/')}/{legacy_user_id}/{old_trade_id}/{shot.get('id')}.bin"
                        data = Path(source_file).read_bytes()
                        digest = hashlib.sha256(data).hexdigest()
                        s3.put_object(Bucket=s3_cfg["bucket"], Key=key, Body=data,
                                      ContentType=shot.get("content_type") or "application/octet-stream",
                                      Metadata={"sha256": digest, "legacy-screenshot-id": str(shot.get("id"))})
                        api_request(base, token, "POST", f"/api/v1/journal/trades/{target_trade['id']}/attachments",
                                    {"attachment_type": str(shot.get("type") or "screenshot").lower(),
                                     "storage_provider": str(s3_cfg.get("provider", "r2")),
                                     "storage_key": key, "original_filename": shot.get("original_filename"),
                                     "content_type": shot.get("content_type"), "byte_size": len(data),
                                     "captured_at": shot.get("created_at"),
                                     "metadata": {"legacy_screenshot_id": str(shot.get("id")),
                                                  "sha256": digest, "source": "TraderOS"}})
                        attachments += 1
                except Exception as exc:
                    report["errors"].append(f"Screenshot migration error: {exc}")
        user_result.update({"imported_trades": imported, "skipped_duplicates": skipped, "psychology_records": psych,
                            "screenshot_records": len(screenshot_rows)})
    report["counts"] = {"users": len(report["users"]), "errors": len(report["errors"])}
    report["complete"] = not report["errors"] and not dry_run
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    ex = sub.add_parser("extract", help="Read-only TraderOS Postgres snapshot")
    ex.add_argument("--dsn", default=os.getenv("TRADEROS_DATABASE_URL"))
    ex.add_argument("--user-id", required=True)
    ex.add_argument("--out", type=Path, required=True)
    pl = sub.add_parser("plan", help="Validate mapping and produce migration plan")
    pl.add_argument("--snapshot", type=Path, required=True)
    pl.add_argument("--config", type=Path, required=True)
    ap = sub.add_parser("apply", help="Apply migration to EdgeNest Journal API")
    ap.add_argument("--snapshot", type=Path, required=True)
    ap.add_argument("--config", type=Path, required=True)
    ap.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.command == "extract":
        if not args.dsn:
            parser.error("--dsn or TRADEROS_DATABASE_URL is required")
        manifest = source_snapshot(args.dsn, args.user_id, args.out)
        print(json.dumps(manifest, indent=2))
        return 0

    config = load_json(args.config)
    if args.command == "plan":
        manifest = load_json(args.snapshot)
        user_id = manifest["source_user_id"]
        user_cfg = config["owner_map"].get(user_id)
        if not user_cfg:
            print(json.dumps({"valid": False, "error": f"Missing owner mapping for {user_id}"}))
            return 2
        state_path = Path(config.get("state_file", "migration-state.json"))
        state = load_json(state_path) if state_path.exists() else {"account_map": {}, "setup_map": {}}
        plan = validate_snapshot(args.snapshot, user_id, state.get("account_map", {}).get(user_id, {}),
                                 state.get("setup_map", {}).get(user_id, {}))
        print(json.dumps(plan, indent=2))
        return 0 if plan["valid"] else 2

    report = migrate(args.snapshot, config, args.dry_run)
    report_path = Path(config.get("report_file", "migration-report.json"))
    dump_json(report_path, report)
    print(json.dumps(report, indent=2))
    return 0 if (args.dry_run and not report["errors"]) or report["complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
