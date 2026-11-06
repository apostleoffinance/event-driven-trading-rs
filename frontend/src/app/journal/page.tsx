export default function JournalPage() {
  return (
    <div>
      <header className="page-header">
        <div><h1>Trading Journal</h1><p>Journal records, broker imports, trade notes, and execution review — separate from EdgeNest OMS.</p></div>
        <span className="badge warn">API integration pending</span>
      </header>
      <div className="grid grid-kpi">
        <section className="panel"><div className="metric"><span className="metric-label">Journal records</span><span className="metric-value">—</span><span className="metric-sub">No live journal source connected</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Imported TraderOS trades</span><span className="metric-value">—</span><span className="metric-sub">Source export not yet available</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Screenshots</span><span className="metric-value">—</span><span className="metric-sub">Storage migration pending</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Journal status</span><span className="metric-value">Schema ready</span><span className="metric-sub">API & authentication integration needed</span></div></section>
      </div>
      <section className="panel" style={{ marginTop: 14 }}>
        <h2 className="panel-title">TRADE HISTORY</h2>
        <div className="journal-empty">
          <h3>No journal data loaded</h3>
          <p>Phase 2 has introduced the owner-scoped persistence repository. The frontend is intentionally not showing fabricated trades. Connect the authenticated REST API and import TraderOS history to populate this table.</p>
          <div className="badge warn">TraderOS Neon database has not been read or imported.</div>
        </div>
      </section>
    </div>
  );
}
