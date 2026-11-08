#!/usr/bin/env python3
"""Read-only TraderOS -> EdgeNest Journal migration verifier.

Run from repository root:
  python scripts/traderos-migration/verify_migration.py --snapshot migration/traderos/snapshot.json --config migration/traderos/config.private.json

Journal records are read via GET only. TokenManager may refresh the access token.
"""
from __future__ import annotations
import argparse, importlib.util, json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("traderos_migrate", HERE / "migrate.py")
if not spec or not spec.loader:
    raise RuntimeError("Unable to load migrate.py")
m = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = m
spec.loader.exec_module(m)

PSY_FIELDS = ("emotion_before","emotion_during","emotion_after","confidence","fear","fomo","frustration","revenge_intensity","boredom","notes")

def unwrap_psy(row):
    """Normalize common API envelope shapes to the psychology record."""
    if not isinstance(row, dict): return row
    for key in ("psychology", "data"):
        nested = row.get(key)
        if isinstance(nested, dict):
            return nested
    return row

def norm_psy(row):
    row = unwrap_psy(row)
    if not isinstance(row, dict): return None
    out = {k: row.get(k) for k in PSY_FIELDS}
    out["revenge_intensity"] = row.get("revenge", row.get("revenge_intensity"))
    return out

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--snapshot", type=Path, required=True)
    ap.add_argument("--config", type=Path, required=True)
    ap.add_argument("--report", type=Path, default=Path("migration-report.json"))
    args = ap.parse_args()
    cfg=m.load_json(args.config); snap=m.load_json(args.snapshot)
    source_user=str(snap["source_user_id"]); base=str(cfg.get("edgenest_api_url","")).rstrip("/")
    tm=m.make_token_manager(cfg, source_user, args.config)
    if not tm: raise RuntimeError("No token mapping found for snapshot source user")
    state_path=Path(cfg.get("state_file","migration-state.json"))
    if not state_path.is_absolute(): state_path=Path.cwd()/state_path
    state=m.load_json(state_path) if state_path.exists() else {}
    accounts=m.api_request(base,tm,"GET","/api/v1/journal/accounts")
    if isinstance(accounts,dict): accounts=accounts.get("accounts",accounts.get("data",[]))
    account_map=state.get("account_map",{}).get(source_user,{})
    if not isinstance(accounts,list): raise RuntimeError(f"Unexpected accounts response: {type(accounts).__name__}")
    source_accounts=m.read_table(args.snapshot,"accounts")
    account_ids=set()
    for row in source_accounts:
        sid=str(row.get("id") or row.get("account_id") or "")
        mapped=account_map.get(sid)
        if mapped: account_ids.add(str(mapped))
    if not account_ids:
        # Fall back to accounts marked as TraderOS imports on destination.
        account_ids={str(a.get("id")) for a in accounts if a.get("source_system")=="TraderOS" and a.get("id")}
    checks=[]; failures=[]; warnings=[]
    def check(name, expected, actual, detail=""):
        ok=expected==actual
        checks.append({"name":name,"status":"PASS" if ok else "FAIL","expected":expected,"actual":actual,"detail":detail})
        if not ok: failures.append(name)
    src_trades=m.read_table(args.snapshot,"trades")
    valid=[t for t in src_trades if not m.has_invalid_trade_chronology(t)]
    quarantined=[t for t in src_trades if m.has_invalid_trade_chronology(t)]
    dest=[]
    for aid in sorted(account_ids):
        offset=0
        while True:
            batch=m.api_request(base,tm,"GET",f"/api/v1/journal/trades?journal_account_id={aid}&limit=500&offset={offset}")
            if isinstance(batch,dict): batch=batch.get("trades",batch.get("data",[]))
            if not isinstance(batch,list): raise RuntimeError("Unexpected trades API response")
            dest.extend(batch)
            if len(batch)<500: break
            offset+=len(batch)
    lineage_rows=[t for t in dest if t.get("source_system")=="TraderOS" and t.get("source_trade_id")]
    lineage_counts={}
    for row in lineage_rows:
        key=str(row.get("source_trade_id"))
        lineage_counts[key]=lineage_counts.get(key,0)+1
    lineage={key: next(t for t in lineage_rows if str(t.get("source_trade_id"))==key) for key in lineage_counts}
    expected_ids={str(t.get("id")) for t in valid}
    actual_ids=set(lineage)
    check("trade lineage IDs",len(expected_ids),len(expected_ids & actual_ids),f"missing={sorted(expected_ids-actual_ids)[:10]}")
    duplicate_rows=sum(count-1 for count in lineage_counts.values() if count>1)
    check("duplicate destination lineage",0,duplicate_rows,f"duplicated source IDs={[k for k,v in lineage_counts.items() if v>1][:10]}")
    check("quarantine count",1,len(quarantined),"Chronology-conflicted trades intentionally excluded")
    src_psy=m.read_table(args.snapshot,"psychology")
    psy_by_trade={str(p.get("trade_id")):p for p in src_psy}
    psy_ok=0; psy_missing=[]
    for sid in expected_ids:
        dt=lineage.get(sid)
        if not dt: continue
        tid=str(dt.get("id"))
        try: dpsy=m.api_request(base,tm,"GET",f"/api/v1/journal/trades/{tid}/psychology")
        except Exception: dpsy=None
        if norm_psy(dpsy)==norm_psy(psy_by_trade.get(sid)): psy_ok+=1
        else: psy_missing.append(sid)
    check("psychology exact match",len(expected_ids),psy_ok,f"mismatch/missing source trade IDs={psy_missing[:10]}")
    src_screens=m.read_table(args.snapshot,"trade_screenshots")
    expected_screens=[s for s in src_screens if str(s.get("trade_id")) in expected_ids]
    attach_ok=0; attach_missing=[]
    for sid in expected_ids:
        dt=lineage.get(sid)
        if not dt: continue
        tid=str(dt.get("id"))
        try: att=m.api_request(base,tm,"GET",f"/api/v1/journal/trades/{tid}/attachments")
        except Exception: att=[]
        if isinstance(att,dict): att=att.get("attachments",att.get("data",[]))
        if not isinstance(att,list): att=[]
        def attachment_meta(a):
            meta=a.get("metadata") if isinstance(a.get("metadata"),dict) else {}
            legacy_id=a.get("legacy_screenshot_id",meta.get("legacy_screenshot_id"))
            sha=a.get("sha256",meta.get("sha256"))
            return (str(legacy_id) if legacy_id is not None else None, str(sha or "").lower())
        bylegacy={legacy_id:a for a in att if (legacy_id:=attachment_meta(a)[0])}
        for s in [x for x in expected_screens if str(x.get("trade_id"))==sid]:
            a=bylegacy.get(str(s.get("id")))
            if a and attachment_meta(a)[1]==str(s.get("file_sha256","")).lower(): attach_ok+=1
            else: attach_missing.append(str(s.get("id")))
    check("screenshot attachment ID+SHA metadata",len(expected_screens),attach_ok,f"missing/mismatch={attach_missing[:10]}")
    report={}
    if args.report.exists():
        try: report=m.load_json(args.report)
        except Exception as e: warnings.append(f"Could not parse report file: {e}")
    archive=report.get("archive",report.get("archive_upload",{})) if isinstance(report,dict) else {}
    if archive: warnings.append("Archive report metadata found; verifier does not download/re-hash archive bytes.")
    else: warnings.append("Archive metadata not found in report; archive validation skipped.")
    result={"status":"PASS" if not failures else "FAIL","checks":checks,"failures":failures,"warnings":warnings,
            "counts":{"source_trades":len(src_trades),"valid_trades":len(valid),"quarantined_trades":len(quarantined),
                      "destination_trades":len(dest),"source_psychology":len(src_psy),"source_screenshots":len(src_screens),
                      "expected_live_screenshots":len(expected_screens)}}
    print(json.dumps(result,indent=2,default=str))
    return 0 if not failures else 1
if __name__=="__main__": raise SystemExit(main())
