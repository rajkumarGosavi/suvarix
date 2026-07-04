//! Exercises the same SQL the `add_equity`/`list_equity`/`update_equity`/`delete_equity`
//! commands run (portfolio::commands, src-tauri/src/portfolio/commands.rs) directly
//! against a real migrated SQLCipher DB — see tests/common/mod.rs for why this
//! doesn't go through `tauri::State`.
mod common;

use rusqlite::params;

#[test]
fn equity_add_list_update_delete_round_trip() {
    let (_dir, state) = common::setup_db_state();
    let conn = state.0.get().unwrap();

    conn.execute(
        "INSERT INTO accounts (name, type) VALUES ('Zerodha', 'broker')",
        [],
    )
    .unwrap();
    let account_id = conn.last_insert_rowid();

    // add_equity
    conn.execute(
        "INSERT INTO equity_holdings (account_id, isin, symbol, exchange, name, quantity, avg_buy_price)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![account_id, "INE000A01000", "FOO", "NSE", "Foo Ltd", 10.0, 100.0],
    ).unwrap();
    let id = conn.last_insert_rowid();
    assert!(id > 0);

    // list_equity
    let symbol: String = conn
        .query_row("SELECT symbol FROM equity_holdings WHERE id = ?1", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(symbol, "FOO");
    let count: i64 = conn
        .query_row("SELECT count(*) FROM equity_holdings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);

    // update_equity
    conn.execute(
        "UPDATE equity_holdings SET symbol=?1, exchange=?2, name=?3, quantity=?4,
         avg_buy_price=?5, updated_at=datetime('now') WHERE id=?6",
        params!["FOO", "NSE", "Foo Ltd", 25.0, 110.0, id],
    ).unwrap();

    let (quantity, avg_buy_price): (f64, f64) = conn
        .query_row(
            "SELECT quantity, avg_buy_price FROM equity_holdings WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(quantity, 25.0);
    assert_eq!(avg_buy_price, 110.0);

    // delete_equity
    conn.execute("DELETE FROM equity_holdings WHERE id=?1", [id]).unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM equity_holdings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
