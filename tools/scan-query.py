"""Read-only queries against a Tessalith scan database. Run with: py -3.13 -I tools/scan-query.py

    scan-query.py "SELECT count(*) FROM rec"     any SQL; tab-separated rows
    scan-query.py --tables                       tables and their columns
    scan-query.py --txst arch_res                texture set records of plugins whose name holds that text
    scan-query.py --self-test                    checks this script on a temp database

--db picks the file; the default is %LOCALAPPDATA%\\sdc\\scan.db. The database opens with mode=ro, so a write fails.
"""
import argparse
import json
import os
import pathlib
import sqlite3
import sys
import tempfile

# Raw txst_slot.slot (TX00..TX07) to 1-based lighting slot. Same table as modscan::plugin_store::TX_TO_NIF, plus one.
TX_TO_SLOT = [1, 2, 6, 3, 4, 5, 7, 8]


def open_ro(db):
    uri = pathlib.Path(db).resolve().as_uri() + "?mode=ro"
    return sqlite3.connect(uri, uri=True)


def tables(conn):
    names = [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")]
    return [(n, [r[1] for r in conn.execute(f'PRAGMA table_info("{n}")')]) for n in names]


def txst_records(conn, plugin_text):
    """One dict per TXST copy in the matching plugins. The key fields are the defining plugin and the 0x FormID."""
    rows = conn.execute(
        """SELECT r.local, r.plugin_id, r.edid, p.name, p.masters_json, w.formid IS NOT NULL
           FROM rec r JOIN plugin p ON p.id = r.plugin_id
           LEFT JOIN rec_winner w ON w.formid = r.formid AND w.plugin_id = r.plugin_id
           WHERE r.sig = 'TXST' AND p.name LIKE ? ORDER BY p.name, r.local""",
        (f"%{plugin_text}%",),
    ).fetchall()
    records = []
    for local, plugin_id, edid, plugin, masters_json, winner in rows:
        masters = json.loads(masters_json)
        index = local >> 24
        slots = {
            TX_TO_SLOT[tx]: path
            for tx, path in conn.execute(
                """SELECT t.slot, x.p FROM txst_slot t JOIN path x ON x.id = t.path_id
                   WHERE t.local = ? AND t.plugin_id = ? ORDER BY t.slot""",
                (local, plugin_id),
            )
        }
        records.append(
            {
                "plugin": plugin,
                "defining": masters[index] if index < len(masters) else plugin,
                "formid": f"0x{local & 0xFFFFFF:X}",
                "edid": edid,
                "winner": bool(winner),
                "slots": dict(sorted(slots.items())),
            }
        )
    return records


def self_test():
    with tempfile.TemporaryDirectory() as tmp:
        db = os.path.join(tmp, "scan.db")
        c = sqlite3.connect(db)
        c.executescript(
            """CREATE TABLE path (id INTEGER PRIMARY KEY, p TEXT NOT NULL UNIQUE);
               CREATE TABLE plugin (id INTEGER PRIMARY KEY, name TEXT NOT NULL, masters_json TEXT NOT NULL);
               CREATE TABLE rec (formid INTEGER, local INTEGER NOT NULL, plugin_id INTEGER NOT NULL, sig TEXT NOT NULL, edid TEXT);
               CREATE TABLE rec_winner (formid INTEGER PRIMARY KEY, plugin_id INTEGER NOT NULL, n_overrides INTEGER NOT NULL);
               CREATE TABLE txst_slot (formid INTEGER, local INTEGER NOT NULL, plugin_id INTEGER NOT NULL, slot INTEGER NOT NULL, path_id INTEGER NOT NULL);
               INSERT INTO path VALUES (1, 'a_d.dds'), (2, 'a_m.dds'), (3, 'a_g.dds');
               INSERT INTO plugin VALUES (1, 'Mine.esp', '["Skyrim.esm"]');
               INSERT INTO rec VALUES (4278190100, 16779284, 1, 'TXST', 'OwnSet'), (74565, 74565, 1, 'TXST', 'Override'), (5, 5, 1, 'ARMO', 'NotASet');
               INSERT INTO rec_winner VALUES (4278190100, 1, 0);
               INSERT INTO txst_slot VALUES (4278190100, 16779284, 1, 0, 1), (4278190100, 16779284, 1, 2, 2), (4278190100, 16779284, 1, 3, 3);"""
        )
        c.commit()
        c.close()
        ro = open_ro(db)
        try:
            found = txst_records(ro, "mine")
            override, own = found
            assert (own["defining"], own["formid"], own["edid"], own["winner"]) == ("Mine.esp", "0x814", "OwnSet", True), own
            # TX02 is the env mask (slot 6) and TX03 the glow (slot 3): the raw index is not the slot.
            assert own["slots"] == {1: "a_d.dds", 3: "a_g.dds", 6: "a_m.dds"}, own
            assert (override["defining"], override["formid"], override["winner"]) == ("Skyrim.esm", "0x12345", False), override
            assert txst_records(ro, "other") == []
            assert ("rec", ["formid", "local", "plugin_id", "sig", "edid"]) in tables(ro)
            try:
                ro.execute("INSERT INTO path VALUES (9, 'x')")
            except sqlite3.OperationalError:
                pass
            else:
                raise AssertionError("a write went through a read-only connection")
        finally:
            ro.close()
    print("self-test ok")


def main():
    default_db = os.path.join(os.environ.get("LOCALAPPDATA", ""), "sdc", "scan.db")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("sql", nargs="?")
    parser.add_argument("--db", default=default_db)
    parser.add_argument("--tables", action="store_true")
    parser.add_argument("--txst", metavar="PLUGIN_TEXT")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    if args.self_test:
        return self_test()
    if not (args.sql or args.tables or args.txst):
        parser.error("give SQL, --tables, --txst or --self-test")
    conn = open_ro(args.db)
    if args.tables:
        for name, columns in tables(conn):
            print(f"{name}: {', '.join(columns)}")
    if args.txst:
        for r in txst_records(conn, args.txst):
            state = "" if r["winner"] else "  (not the winner)"
            print(f"{r['defining']} {r['formid']} {r['edid']} [in {r['plugin']}]{state}")
            for slot, path in r["slots"].items():
                print(f"  slot{slot}  {path}")
    if args.sql:
        for row in conn.execute(args.sql):
            print("\t".join("" if v is None else str(v) for v in row))


if __name__ == "__main__":
    main()
