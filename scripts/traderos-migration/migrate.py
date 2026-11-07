#!/usr/bin/env python3
"""TraderOS -> EdgeNest Journal migration utility.

Source PostgreSQL is read-only. The destination is written exclusively via the
authenticated EdgeNest Journal API. Credentials are supplied through environment
variables/private config and are never written to migration state or reports.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import time
from datetime import date, datetime
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any
from urllib.parse import quote

SOURCE_TABLES = (
    "accounts", "account_risk_profiles", "setups", "trades", "psychology",
    "checklist_templates", "checklist_items", "trade_checklist_responses",
    "trade_screenshots",
)
SECRETISH = re.compile(r"(password|token|secret|credential|private.?key|refresh.?token)", re.I)
SOURCE_FORMAT = "traderos-edgenest-migration-v2"


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
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(json_value(obj), indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    tmp.replace(path)


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
    """Export a selected user and related journal facts from a read-only snapshot."""
    conn = connect_source(dsn)
    files_dir = out.with_suffix("").with_name(out.stem + "_screenshot_files")
    counts: dict[str, int] = {}
    screenshot_manifest: list[dict[str, Any]] = []
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
                    sql = """SELECT s.* FROM trade_screenshots s JOIN trades t ON t.id=s.trade_id
                             WHERE s.user_id=%s AND t.user_id=%s"""
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
                            digest = hashlib.sha256(binary).hexdigest()
                            item.update(file_path=str(target), file_sha256=digest, file_size=len(binary))
                        screenshot_manifest.append({
                            "id": key, "trade_id": str(item.get("trade_id")),
                            "has_binary": bool(item.get("file_path")),
                            "sha256": item.get("file_sha256"),
                            "byte_size": item.get("file_size"),
                            "remote_key": item.get("storage_key") or item.get("file_path"),
                        })
                    clean_rows.append(json_value(item))
                dump_json(out.with_suffix("").with_name(f"{out.stem}_{table}.json"), clean_rows)
                counts[table] = len(clean_rows)
    finally:
        conn.close()
    manifest = {
        "format": SOURCE_FORMAT,
        "created_at": datetime.now().astimezone().isoformat(),
        "source_user_id": str(legacy_user_id),
        "tables": counts,
        "screenshots": screenshot_manifest,
        "screenshot_files_dir": str(files_dir) if files_dir.exists() else None,
        "snapshot_prefix": out.stem,
    }
    dump_json(out, manifest)
    return manifest


def parse_time(value: Any) -> str | None:
    if value is None:
        return None
    dt = value if isinstance(value, datetime) else datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    if dt.tzinfo is None:
        raise ValueError(f"Naive timestamp encountered: {value!r}; source timezone cannot be inferred")
    return dt.isoformat()


def decimal_str(value: Any, default: str | None = None) -> str | None:
    if value is None:
        return default
    try:
        return str(Decimal(str(value)))
    except InvalidOperation as exc:
        raise ValueError(f"Invalid decimal value: {value!r}") from exc


def api_request(base: str, token: str, method: str, path: str, body: Any = None, retries: int = 5) -> Any:
    try:
        import requests
    except ImportError as exc:
        raise SystemExit("Missing requests. Install scripts/traderos-migration/requirements.txt") from exc
    url = base.rstrip("/") + path
    for attempt in range(retries):
        response = requests.request(
            method, url,
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
            json=body, timeout=(10, 90),
        )
        if response.status_code in (429, 502, 503, 504) and attempt < retries - 1:
            time.sleep(min(2 ** attempt, 20))
            continue
        if response.status_code >= 400:
            # Never include response body: gateways sometimes echo sensitive request data.
            raise RuntimeError(f"{method} {path} failed with HTTP {response.status_code}")
        if response.status_code == 204 or not response.content:
            return None
        try:
            return response.json()
        except ValueError as exc:
            raise RuntimeError(f"{method} {path} returned invalid JSON") from exc
    raise RuntimeError(f"Retry exhaustion for {method} {path}")


def read_table(prefix: Path, table: str) -> list[dict[str, Any]]:
    path = prefix.with_suffix("").with_name(f"{prefix.stem}_{table}.json")
    return load_json(path) if path.exists() else []


def make_trade(row: dict[str, Any], account_id: str, setup_map: dict[str, str],
               legacy_user_id: str, pnl_currency: str = "USD") -> dict[str, Any]:
    legacy_id = str(row["id"])
    opened = parse_time(row.get("trade_timestamp") or row.get("entry_timestamp"))
    if not opened:
        raise ValueError(f"Trade {legacy_id} has no entry timestamp")
    closed = parse_time(row.get("exit_timestamp"))
    status_src = str(row.get("status") or row.get("result") or "open").lower()
    closed_trade = status_src in {"closed", "complete", "completed", "win", "loss", "breakeven"} or closed is not None
    status = "closed" if closed_trade else "open"
    if status == "closed" and (not closed or row.get("exit_price") is None):
        raise ValueError(f"Closed trade {legacy_id} has missing exit timestamp or exit price")
    direction = str(row.get("direction") or "").lower()
    if direction not in ("long", "short"):
        raise ValueError(f"Unsupported or missing trade direction {direction!r} for trade {legacy_id}")
    setup_id = setup_map.get(str(row.get("setup_id")) ) if row.get("setup_id") else None
    commission = Decimal(str(row.get("commission") or 0))
    swap = Decimal(str(row.get("swap") or 0))
    net_value = row.get("realized_pnl")
    gross = (Decimal(str(net_value)) - commission - swap) if net_value is not None else None
    quantity = decimal_str(row.get("lot_size") or row.get("quantity"))
    entry_price = decimal_str(row.get("entry_price"))
    if not quantity or Decimal(quantity) <= 0:
        raise ValueError(f"Trade {legacy_id} has missing or non-positive quantity")
    if not entry_price or Decimal(entry_price) <= 0:
        raise ValueError(f"Trade {legacy_id} has missing or non-positive entry price")
    source_payload = {
        "legacy_user_id": legacy_user_id, "legacy_trade_id": legacy_id,
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
        "created_at": row.get("created_at"), "updated_at": row.get("updated_at"),
    }
    return {
        "journal_account_id": account_id, "setup_id": setup_id,
        "symbol": str(row.get("symbol") or row.get("symbol_raw") or "").upper(),
        "direction": direction, "status": status, "source": "legacy_import",
        "source_system": "TraderOS", "source_trade_id": legacy_id,
        "opened_at": opened, "closed_at": closed if status == "closed" else None,
        "quantity": quantity, "entry_price": entry_price,
        "exit_price": decimal_str(row.get("exit_price")),
        "stop_loss": decimal_str(row.get("stop_loss")), "take_profit": decimal_str(row.get("take_profit")),
        "gross_pnl": decimal_str(gross), "commission": decimal_str(commission),
        "swap": decimal_str(swap), "other_fees": decimal_str(row.get("other_fees"), "0"),
        "net_pnl": decimal_str(net_value), "pnl_currency": str(pnl_currency or "USD").upper(),
        "risk_amount": decimal_str(row.get("risk_amount")), "r_multiple": decimal_str(row.get("realized_r")),
        "session": row.get("session"), "timeframe": row.get("timeframe"),
        "timezone": row.get("timezone"), "mfe_r": decimal_str(row.get("mfe_r")),
        "mae_r": decimal_str(row.get("mae_r")), "notes": row.get("notes"),
        "tags": row.get("tags") if isinstance(row.get("tags"), list) else [],
        "setup_valid": row.get("setup_valid"), "rules_followed": row.get("rules_followed"),
        "emotional_trade": row.get("emotional_trade"), "mistake": row.get("mistake"),
        "mistake_notes": row.get("mistake_notes"), "discipline_score": row.get("discipline_score"),
        "acknowledged_warnings": row.get("acknowledged_warnings") if isinstance(row.get("acknowledged_warnings"), list) else [],
        "source_payload": source_payload,
    }


def checklist_by_trade(prefix: Path, trade_ids: set[str]) -> dict[str, list[dict[str, Any]]]:
    items = {str(x.get("id")): x for x in read_table(prefix, "checklist_items")}
    templates = {str(x.get("id")): x for x in read_table(prefix, "checklist_templates")}
    result: dict[str, list[dict[str, Any]]] = {}
    for response in read_table(prefix, "trade_checklist_responses"):
        trade_id = str(response.get("trade_id"))
        if trade_id not in trade_ids:
            continue
        item = items.get(str(response.get("item_id")), {})
        template = templates.get(str(item.get("template_id")), {})
        result.setdefault(trade_id, []).append({
            "legacy_item_id": str(response.get("item_id")),
            "label": item.get("label"), "description": item.get("description"),
            "category": item.get("category"), "kind": item.get("kind"),
            "auto_key": item.get("auto_key"), "required": item.get("required"),
            "sort_order": item.get("sort_order"), "template_name": template.get("name"),
            "template_id": str(item.get("template_id")) if item.get("template_id") else None,
            "checked": bool(response.get("checked")),
        })
    return result


def validate_snapshot(prefix: Path, legacy_user_id: str, account_map: dict[str, str],
                      setup_map: dict[str, str], allow_placeholders: bool = False) -> dict[str, Any]:
    manifest = load_json(prefix)
    errors: list[str] = []
    warnings: list[str] = []
    accounts = read_table(prefix, "accounts")
    setups = read_table(prefix, "setups")
    trades = read_table(prefix, "trades")
    psych = read_table(prefix, "psychology")
    screenshots = read_table(prefix, "trade_screenshots")
    if manifest.get("format") not in ("traderos-edgenest-migration-v1", SOURCE_FORMAT):
        errors.append("Unrecognized or missing snapshot format")
    if str(manifest.get("source_user_id")) != str(legacy_user_id):
        errors.append("Snapshot owner does not match selected owner mapping")
    if not accounts:
        errors.append("No source accounts found for selected TraderOS user")
    source_account_ids = {str(a.get("id")) for a in accounts}
    source_trade_ids = {str(t.get("id")) for t in trades}
    if not trades:
        warnings.append("No source trades found; account-only migration is possible")
    for trade in trades:
        if str(trade.get("account_id")) not in source_account_ids:
            errors.append(f"Trade {trade.get('id')} references missing source account")
        if str(trade.get("account_id")) not in account_map or (not allow_placeholders and str(account_map.get(str(trade.get("account_id")), "")).startswith("DRYRUN:")):
            errors.append(f"Missing destination account mapping for source account {trade.get('account_id')}")
        try:
            make_trade(trade, account_map.get(str(trade.get("account_id")), ""), setup_map, legacy_user_id)
        except Exception as exc:
            errors.append(str(exc))
        if trade.get("setup_id") and str(trade.get("setup_id")) not in {str(s.get("id")) for s in setups}:
            errors.append(f"Trade {trade.get('id')} references missing setup")
    for setup in setups:
        if str(setup.get("id")) not in setup_map or (not allow_placeholders and str(setup_map.get(str(setup.get("id")), "")).startswith("DRYRUN:")):
            errors.append(f"Missing destination setup mapping for setup {setup.get('id')}")
    for row in psych:
        if str(row.get("trade_id")) not in source_trade_ids:
            errors.append(f"Psychology record references missing trade {row.get('trade_id')}")
    for row in screenshots:
        if str(row.get("trade_id")) not in source_trade_ids:
            errors.append(f"Screenshot references missing trade {row.get('trade_id')}")
        file_path = row.get("file_path")
        if file_path and Path(file_path).is_file():
            data = Path(file_path).read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            if row.get("file_sha256") and digest != row["file_sha256"]:
                errors.append(f"Screenshot checksum mismatch for {row.get('id')}")
        elif not row.get("storage_key"):
            warnings.append(f"Screenshot {row.get('id')} has no binary file or remote storage key")
    return {
        "legacy_user_id": legacy_user_id, "accounts": len(accounts), "setups": len(setups),
        "trades": len(trades), "psychology": len(psych), "screenshots": len(screenshots),
        "checklist_responses": len(read_table(prefix, "trade_checklist_responses")),
        "errors": errors, "warnings": warnings, "valid": not errors,
    }


def migrate(prefix: Path, config: dict[str, Any], dry_run: bool) -> dict[str, Any]:
    manifest = load_json(prefix)
    source_user = str(manifest["source_user_id"])
    owner_map = config.get("owner_map", {})
    user_cfg = owner_map.get(source_user)
    report: dict[str, Any] = {
        "mode": "dry-run" if dry_run else "apply", "source": "TraderOS",
        "snapshot_format": manifest.get("format"), "source_user_id": source_user,
        "users": [], "errors": [], "warnings": [], "complete": False,
    }
    if not user_cfg or not user_cfg.get("jwt"):
        report["errors"].append("Missing explicit EdgeNest owner/JWT mapping for snapshot user")
        return report
    token = user_cfg["jwt"]
    base = str(config.get("edgenest_api_url", "")).strip()
    if not base.startswith("https://") and not base.startswith("http://localhost"):
        report["errors"].append("EdgeNest API URL must use HTTPS (localhost is allowed for tests)")
        return report
    state_path = Path(config.get("state_file", "migration-state.json"))
    state = load_json(state_path) if state_path.exists() else {"account_map": {}, "setup_map": {}, "screenshot_map": {}}
    state = {**{"account_map": {}, "setup_map": {}, "screenshot_map": {}}, **state}
    acc_map = state["account_map"].setdefault(source_user, {})
    setup_map = state["setup_map"].setdefault(source_user, {})
    accounts = [a for a in read_table(prefix, "accounts") if str(a.get("user_id")) == source_user]
    setups = [s for s in read_table(prefix, "setups") if str(s.get("user_id")) == source_user]
    trades = [t for t in read_table(prefix, "trades") if str(t.get("user_id")) == source_user]
    risk_profiles = {str(r.get("account_id")): r for r in read_table(prefix, "account_risk_profiles")}
    user_plan = {"legacy_user_id": source_user, "accounts": len(accounts), "setups": len(setups),
                 "trades": len(trades), "psychology": len(read_table(prefix, "psychology")),
                 "screenshots": len(read_table(prefix, "trade_screenshots"))}
    if dry_run:
        # Dry-run never mutates checkpoint state or calls the destination API.
        effective_accounts = {**acc_map, **{str(a["id"]): f"DRYRUN:{a['id']}" for a in accounts if str(a["id"]) not in acc_map}}
        effective_setups = {**setup_map, **{str(s["id"]): f"DRYRUN:{s['id']}" for s in setups if str(s["id"]) not in setup_map}}
        plan = validate_snapshot(prefix, source_user, effective_accounts, effective_setups, allow_placeholders=True)
        report["users"].append({"plan": user_plan, "validation": plan})
        report["warnings"].extend(plan["warnings"])
        report["errors"].extend(plan["errors"])
        report["complete"] = not report["errors"]
        return report

    # Destination creates are idempotent using external source IDs. Persist checkpoints only after success.
    for account in accounts:
        old_id = str(account["id"])
        if old_id in acc_map:
            continue
        profile = risk_profiles.get(old_id, {})
        broker = " / ".join(x for x in [str(account.get("firm") or "").strip(),
                                         str(account.get("program") or "").strip()] if x) or None
        body = {
            "name": str(account.get("account_name") or "TraderOS account"),
            "broker": broker, "account_type": "funded" if account.get("firm") else "journal",
            "currency": str(account.get("currency") or "USD").upper(),
            "initial_balance": decimal_str(account.get("starting_balance"), "0"),
            "external_source": "TraderOS", "external_account_id": old_id,
            "migration_metadata": {
                "legacy_account_id": old_id, "legacy_firm": account.get("firm"),
                "legacy_program": account.get("program"), "legacy_status": account.get("status"),
                "legacy_current_balance": decimal_str(account.get("current_balance")),
                "legacy_current_equity": decimal_str(account.get("current_equity")),
                "legacy_risk_profile": json_value(profile),
            },
        }
        result = api_request(base, token, "POST", "/api/v1/journal/accounts", body)
        acc_map[old_id] = result["data"]["id"]
        dump_json(state_path, state)

    for setup in setups:
        old_id = str(setup["id"])
        if old_id in setup_map:
            continue
        body = {
            "name": str(setup.get("name") or f"Imported setup {old_id}")[:250],
            "description": setup.get("description"),
            "rules_json": {"traderos_setup_id": old_id, "active": setup.get("active", True),
                           "source_metadata": json_value(setup)},
        }
        result = api_request(base, token, "POST", "/api/v1/journal/setups", body)
        setup_map[old_id] = result["data"]["id"]
        dump_json(state_path, state)

    plan = validate_snapshot(prefix, source_user, acc_map, setup_map)
    report["users"].append({"plan": user_plan, "validation": plan})
    report["warnings"].extend(plan["warnings"])
    report["errors"].extend(plan["errors"])
    if report["errors"]:
        return report

    trades_by_account: dict[str, list[dict[str, Any]]] = {}
    for row in trades:
        trades_by_account.setdefault(str(row["account_id"]), []).append(row)
    checks = checklist_by_trade(prefix, {str(t["id"]) for t in trades})
    trade_lookup: dict[str, dict[str, Any]] = {}
    for account_id, rows in trades_by_account.items():
        account = next(a for a in accounts if str(a["id"]) == account_id)
        for start in range(0, len(rows), 10000):
            body_trades = []
            for trade_row in rows[start:start + 10000]:
                body = make_trade(trade_row, acc_map[account_id], setup_map, source_user,
                                  str(account.get("currency") or "USD"))
                body["checklist"] = {"legacy_traderos_responses": checks.get(str(trade_row["id"]), [])}
                body_trades.append(body)
            result = api_request(base, token, "POST", "/api/v1/journal/import",
                                 {"journal_account_id": acc_map[account_id],
                                  "source_system": "TraderOS", "trades": body_trades})
            report.setdefault("trade_batches", []).append(result.get("data", {}))
        # Ensure retries can locate each imported trade by source ID and account, avoiding ID collisions across accounts.
        found_rows = []
        offset = 0
        while True:
            page = api_request(base, token, "GET",
                               f"/api/v1/journal/trades?journal_account_id={quote(acc_map[account_id])}&limit=500&offset={offset}")
            found_rows.extend(page.get("data", []))
            if len(page.get("data", [])) < 500:
                break
            offset += 500
        for row in found_rows:
            if row.get("source_system") == "TraderOS" and row.get("source_trade_id"):
                trade_lookup[f"{account_id}:{row['source_trade_id']}"] = row

    for psych_row in read_table(prefix, "psychology"):
        old_trade_id = str(psych_row.get("trade_id"))
        source_account = next((t.get("account_id") for t in trades if str(t.get("id")) == old_trade_id), None)
        target = trade_lookup.get(f"{source_account}:{old_trade_id}")
        if not target:
            report["errors"].append(f"Could not resolve imported psychology trade {old_trade_id}")
            continue
        payload = {
            "emotion_before": psych_row.get("emotion_before"),
            "emotion_during": psych_row.get("emotion_during"),
            "emotion_after": psych_row.get("emotion_after"),
            "confidence": psych_row.get("confidence"), "fear": psych_row.get("fear"),
            "fomo": psych_row.get("fomo"), "frustration": psych_row.get("frustration"),
            "revenge_intensity": psych_row.get("revenge", psych_row.get("revenge_intensity")),
            "boredom": psych_row.get("boredom"), "notes": psych_row.get("notes"),
        }
        api_request(base, token, "PUT",
                    f"/api/v1/journal/trades/{quote(target['id'])}/psychology", payload)
        report["psychology_imported"] = report.get("psychology_imported", 0) + 1

    screenshot_rows = read_table(prefix, "trade_screenshots")
    s3_cfg = config.get("object_storage", {})
    if screenshot_rows and not (s3_cfg.get("bucket") and s3_cfg.get("access_key_id") and s3_cfg.get("secret_access_key")):
        report["warnings"].append(f"{len(screenshot_rows)} screenshot(s) extracted but not transferred: destination object storage is not configured")
    elif screenshot_rows:
        try:
            import boto3
            client = boto3.client(
                "s3", endpoint_url=s3_cfg.get("endpoint_url"), region_name=s3_cfg.get("region", "auto"),
                aws_access_key_id=s3_cfg["access_key_id"], aws_secret_access_key=s3_cfg["secret_access_key"],
            )
            for shot in screenshot_rows:
                old_trade_id = str(shot.get("trade_id"))
                source_trade = next((t for t in trades if str(t.get("id")) == old_trade_id), None)
                target = trade_lookup.get(f"{source_trade.get('account_id')}:{old_trade_id}") if source_trade else None
                if not target:
                    report["errors"].append(f"Screenshot {shot.get('id')} has no destination trade mapping")
                    continue
                file_name = shot.get("file_path")
                if not file_name or not Path(file_name).is_file():
                    report["warnings"].append(f"Screenshot {shot.get('id')} has metadata only; no extracted binary available")
                    continue
                data = Path(file_name).read_bytes()
                digest = hashlib.sha256(data).hexdigest()
                if shot.get("file_sha256") and digest != shot["file_sha256"]:
                    report["errors"].append(f"Screenshot checksum changed before upload: {shot.get('id')}")
                    continue
                key = f"{s3_cfg.get('prefix', 'traderos-import').strip('/')}/{source_user}/{old_trade_id}/{shot.get('id')}"
                client.put_object(Bucket=s3_cfg["bucket"], Key=key, Body=data,
                                  ContentType=shot.get("content_type") or "application/octet-stream",
                                  Metadata={"sha256": digest, "legacy-screenshot-id": str(shot.get("id"))})
                head = client.head_object(Bucket=s3_cfg["bucket"], Key=key)
                remote_sha = (head.get("Metadata") or {}).get("sha256")
                if remote_sha != digest:
                    raise RuntimeError(f"Destination object checksum verification failed for screenshot {shot.get('id')}")
                api_request(base, token, "POST", f"/api/v1/journal/trades/{quote(target['id'])}/attachments",
                            {"attachment_type": str(shot.get("type") or "screenshot").lower(),
                             "storage_provider": str(s3_cfg.get("provider", "r2")),
                             "storage_key": key, "original_filename": shot.get("original_filename"),
                             "content_type": shot.get("content_type"), "byte_size": len(data),
                             "captured_at": parse_time(shot.get("created_at")),
                             "metadata": {"legacy_screenshot_id": str(shot.get("id")), "sha256": digest, "source": "TraderOS"}})
                report["screenshots_imported"] = report.get("screenshots_imported", 0) + 1

        except Exception as exc:
            report["errors"].append(f"Screenshot transfer failed: {type(exc).__name__}")
    report["complete"] = not report["errors"] and not report["warnings"]
    report["source_counts"] = manifest.get("tables", {})
    report["imported_counts"] = {
        "trades": sum(int(x.get("imported_count", 0)) for x in report.get("trade_batches", [])),
        "duplicates": sum(int(x.get("skipped_count", 0)) for x in report.get("trade_batches", [])),
        "psychology": report.get("psychology_imported", 0),
        "screenshots": report.get("screenshots_imported", 0),
    }
    dump_json(state_path, state)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    ex = sub.add_parser("extract", help="Read-only TraderOS PostgreSQL snapshot")
    ex.add_argument("--dsn", default=os.getenv("TRADEROS_DATABASE_URL"))
    ex.add_argument("--user-id", required=True)
    ex.add_argument("--out", type=Path, required=True)
    for name in ("plan", "apply"):
        cmd = sub.add_parser(name)
        cmd.add_argument("--snapshot", type=Path, required=True)
        cmd.add_argument("--config", type=Path, required=True)
        if name == "apply":
            cmd.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.command == "extract":
        if not args.dsn:
            parser.error("--dsn or TRADEROS_DATABASE_URL is required")
        print(json.dumps(source_snapshot(args.dsn, args.user_id, args.out), indent=2))
        return 0
    config = load_json(args.config)
    prefix = args.snapshot
    if args.command == "plan":
        manifest = load_json(prefix)
        user_id = str(manifest["source_user_id"])
        state_path = Path(config.get("state_file", "migration-state.json"))
        state = load_json(state_path) if state_path.exists() else {"account_map": {}, "setup_map": {}}
        plan = validate_snapshot(prefix, user_id, state.get("account_map", {}).get(user_id, {}),
                                 state.get("setup_map", {}).get(user_id, {}))
        owner_ok = bool(config.get("owner_map", {}).get(user_id, {}).get("jwt"))
        plan["owner_mapping_present"] = owner_ok
        plan["valid"] = plan["valid"] and owner_ok
        print(json.dumps(plan, indent=2))
        return 0 if plan["valid"] else 2
    report = migrate(prefix, config, args.dry_run)
    dump_json(Path(config.get("report_file", "migration-report.json")), report)
    print(json.dumps(report, indent=2))
    return 0 if (args.dry_run and not report["errors"]) or report["complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
