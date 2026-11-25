"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useOrganization } from "@/features/organization/OrganizationProvider";
import { useWorkspace, type Workspace } from "@/features/workspace/WorkspaceProvider";
import { apiRequest } from "@/lib/api/client";

type WorkspaceGrant = {
  userId: string;
  role: "WORKSPACE_ADMIN" | "TRADER" | "RISK_MANAGER" | "VIEWER";
  status: "ACTIVE" | "REVOKED";
  revision: number;
  reason: string;
  createdAt: string;
  updatedAt: string;
};

type OrganizationMember = { userId: string; role: string; status: string; revision: number };
type Envelope<T> = { data: T };
const roles: WorkspaceGrant["role"][] = ["WORKSPACE_ADMIN", "TRADER", "RISK_MANAGER", "VIEWER"];

function slugify(value: string) {
  return value.trim().toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 63).replace(/-+$/g, "");
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : "The request could not be completed. Please try again.";
}

export default function WorkspaceSettingsPage() {
  const { activeOrganization, activeOrganizationId } = useOrganization();
  const { workspaces, activeWorkspace, activeWorkspaceId, loading, error, refreshWorkspaces, selectWorkspace } = useWorkspace();
  const isOrgAdmin = activeOrganization?.membershipRole === "OWNER" || activeOrganization?.membershipRole === "ADMIN";
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [grants, setGrants] = useState<WorkspaceGrant[]>([]);
  const [members, setMembers] = useState<OrganizationMember[]>([]);
  const [grantsLoading, setGrantsLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [rename, setRename] = useState("");
  const [newUserId, setNewUserId] = useState("");
  const [newRole, setNewRole] = useState<WorkspaceGrant["role"]>("VIEWER");
  const [reason, setReason] = useState("");

  useEffect(() => {
    if (!activeOrganizationId || !isOrgAdmin) { setMembers([]); return; }
    let current = true;
    void apiRequest<Envelope<OrganizationMember[]>>(`/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/members`, { headers: { "X-Organization-Id": activeOrganizationId } })
      .then((response) => { if (current) setMembers(response.data ?? []); })
      .catch(() => { if (current) setMembers([]); });
    return () => { current = false; };
  }, [activeOrganizationId, isOrgAdmin]);

  const grantableMembers = members.filter((member) => member.status === "ACTIVE" && !["OWNER", "ADMIN"].includes(member.role) && !grants.some((grant) => grant.userId === member.userId));





  const selectedWorkspace = useMemo(
    () => workspaces.find((workspace) => workspace.id === selectedId) ?? activeWorkspace,
    [workspaces, selectedId, activeWorkspace],
  );

  const loadGrants = useCallback(async (workspace: Workspace) => {
    if (!activeOrganizationId || !isOrgAdmin) {
      setGrants([]);
      return;
    }
    setGrantsLoading(true);
    try {
      const result = await apiRequest<Envelope<WorkspaceGrant[]>>(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(workspace.id)}/members`,
        { headers: { "X-Organization-Id": activeOrganizationId } },
      );
      setGrants(result.data ?? []);
      setFailure(null);
    } catch (cause) {
      setFailure(errorMessage(cause));
      setGrants([]);
    } finally {
      setGrantsLoading(false);
    }
  }, [activeOrganizationId, isOrgAdmin]);

  useEffect(() => {
    if (activeWorkspaceId && !selectedId) setSelectedId(activeWorkspaceId);
  }, [activeWorkspaceId, selectedId]);

  useEffect(() => {
    if (selectedId && workspaces.some((workspace) => workspace.id === selectedId) && selectedId !== activeWorkspaceId) {
      selectWorkspace(selectedId);
    }
  }, [selectedId, activeWorkspaceId, workspaces, selectWorkspace]);

  useEffect(() => {
    if (!selectedWorkspace) {
      setGrants([]);
      return;
    }
    setRename(selectedWorkspace.name);
    void loadGrants(selectedWorkspace);
  }, [selectedWorkspace, loadGrants]);

  async function runAction(action: () => Promise<void>, success: string) {
    setBusy(true);
    setMessage(null);
    setFailure(null);
    try {
      await action();
      setMessage(success);
    } catch (cause) {
      setFailure(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function createWorkspace(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeOrganizationId) return;
    const name = newName.trim();
    const slug = slugify(name);
    if (!name || !slug) {
      setFailure("Enter a workspace name containing letters or numbers.");
      return;
    }
    await runAction(async () => {
      const result = await apiRequest<Envelope<Workspace>>(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces`,
        {
          method: "POST",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ name, slug, reason: "Created from workspace settings" }),
        },
      );
      await refreshWorkspaces();
      if (result.data?.id) {
        setSelectedId(result.data.id);
      }
      setNewName("");
    }, "Workspace created.");
  }

  async function renameWorkspace(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeOrganizationId || !selectedWorkspace) return;
    const name = rename.trim();
    if (!name || name === selectedWorkspace.name) return;
    await runAction(async () => {
      await apiRequest<Envelope<Workspace>>(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(selectedWorkspace.id)}`,
        {
          method: "PATCH",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ expected_revision: selectedWorkspace.revision, name, reason: reason.trim() || "Workspace renamed from settings" }),
        },
      );
      await refreshWorkspaces();
    }, "Workspace name updated.");
  }

  async function setDefault(workspace: Workspace) {
    if (!activeOrganizationId) return;
    await runAction(async () => {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(workspace.id)}/default`,
        {
          method: "POST",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ expected_revision: workspace.revision, reason: reason.trim() || "Default workspace changed from settings" }),
        },
      );
      await refreshWorkspaces();
    }, "Default workspace updated.");
  }

  async function archiveWorkspace(workspace: Workspace) {
    if (!activeOrganizationId) return;
    const confirmed = window.confirm(`Archive “${workspace.name}”? This cannot be undone from the workspace interface.`);
    if (!confirmed) return;
    await runAction(async () => {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(workspace.id)}/archive`,
        {
          method: "POST",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ expected_revision: workspace.revision, reason: reason.trim() || "Workspace archived from settings" }),
        },
      );
      setSelectedId(null);
      await refreshWorkspaces();
    }, "Workspace archived.");
  }

  async function addGrant(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeOrganizationId || !selectedWorkspace) return;
    const userId = newUserId.trim();
    if (!userId) return;
    await runAction(async () => {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(selectedWorkspace.id)}/members`,
        {
          method: "POST",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ user_id: userId, role: newRole, reason: reason.trim() || "Workspace access granted from settings" }),
        },
      );
      setNewUserId("");
      await loadGrants(selectedWorkspace);
    }, "Workspace access granted.");
  }

  async function updateGrant(grant: WorkspaceGrant, patch: { role?: WorkspaceGrant["role"]; status?: WorkspaceGrant["status"] }) {
    if (!activeOrganizationId || !selectedWorkspace) return;
    await runAction(async () => {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces/${encodeURIComponent(selectedWorkspace.id)}/members/${encodeURIComponent(grant.userId)}`,
        {
          method: "PATCH",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({
            expected_revision: grant.revision,
            role: patch.role ?? grant.role,
            status: patch.status ?? grant.status,
            reason: reason.trim() || "Workspace access updated from settings",
          }),
        },
      );
      await loadGrants(selectedWorkspace);
    }, "Workspace access updated.");
  }

  if (!activeOrganizationId) {
    return <section className="panel workspace-settings"><h1>Workspace settings</h1><p className="muted">Choose an active organization before managing workspaces.</p></section>;
  }

  return (
    <div className="workspace-settings">
      <header className="page-header">
        <div>
          <span className="eyebrow">ORGANIZATION CONTROL PLANE</span>
          <h1>Workspace settings</h1>
          <p>Manage operational workspaces, default selection and member access.</p>
        </div>
        <button className="btn" type="button" onClick={() => void refreshWorkspaces()} disabled={loading || busy}>Refresh</button>
      </header>

      {message ? <div className="workspace-notice" role="status">{message}</div> : null}
      {failure || error ? <div className="error" role="alert">{failure ?? error}</div> : null}

      <section className="panel">
        <div className="workspace-section-heading">
          <div><h2>Workspaces</h2><p className="muted">Only workspaces you are authorized to access are listed.</p></div>
          <span className="workspace-count">{workspaces.length} active</span>
        </div>
        {loading ? <div className="loading" role="status">Loading accessible workspaces…</div> : workspaces.length === 0 ? (
          <div className="empty">No active workspaces are available to this account.</div>
        ) : (
          <div className="workspace-list">
            {workspaces.map((workspace) => (
              <button key={workspace.id} type="button" className={`workspace-choice${selectedWorkspace?.id === workspace.id ? " selected" : ""}`} onClick={() => { setSelectedId(workspace.id); selectWorkspace(workspace.id); }} aria-pressed={selectedWorkspace?.id === workspace.id}>
                <span className="workspace-choice-main"><strong>{workspace.name}</strong><small>{workspace.slug}</small></span>
                <span className="workspace-choice-meta">{workspace.isDefault ? <span className="workspace-tag">DEFAULT</span> : null}{activeWorkspaceId === workspace.id ? <span className="workspace-tag current">ACTIVE</span> : null}</span>
              </button>
            ))}
          </div>
        )}
      </section>

      {isOrgAdmin ? (
        <section className="panel">
          <h2>Create workspace</h2>
          <p className="muted">A new workspace starts empty. Existing accounts are not moved automatically.</p>
          <form className="workspace-form" onSubmit={createWorkspace}>
            <label className="journal-field"><span>Workspace name</span><input value={newName} onChange={(event) => setNewName(event.target.value)} maxLength={120} required placeholder="e.g. Systematic Strategies" /></label>
            <label className="journal-field"><span>Workspace slug (generated)</span><input value={slugify(newName)} readOnly aria-readonly="true" placeholder="workspace-slug" /></label>
            <button className="btn primary" type="submit" disabled={busy || !newName.trim()}>Create workspace</button>
          </form>
        </section>
      ) : null}

      {selectedWorkspace ? (
        <section className="panel">
          <div className="workspace-section-heading"><div><h2>{selectedWorkspace.name}</h2><p className="muted">Workspace ID: <code>{selectedWorkspace.id}</code></p></div><span className="workspace-tag">{selectedWorkspace.isDefault ? "DEFAULT" : "ACTIVE"}</span></div>
          <div className="workspace-detail-grid">
            <div><span>Stable slug</span><strong>{selectedWorkspace.slug}</strong></div>
            <div><span>Revision</span><strong>{selectedWorkspace.revision}</strong></div>
            <div><span>Organization</span><strong>{activeOrganization?.name ?? "Active organization"}</strong></div>
          </div>
          {isOrgAdmin ? (
            <>
              <form className="workspace-form workspace-rename-form" onSubmit={renameWorkspace}>
                <label className="journal-field"><span>Workspace name</span><input value={rename} onChange={(event) => setRename(event.target.value)} maxLength={120} required /></label>
                <button className="btn" type="submit" disabled={busy || !rename.trim() || rename.trim() === selectedWorkspace.name}>Save name</button>
                <button className="btn" type="button" disabled={busy || selectedWorkspace.isDefault} onClick={() => void setDefault(selectedWorkspace)}>Set as default</button>
                <button className="btn danger" type="button" disabled={busy || selectedWorkspace.isDefault} onClick={() => void archiveWorkspace(selectedWorkspace)}>Archive</button>
              </form>
              <label className="journal-field workspace-reason"><span>Reason for administrative changes (optional)</span><input value={reason} onChange={(event) => setReason(event.target.value)} maxLength={500} placeholder="Audit reason recorded by the API" /></label>
            </>
          ) : <p className="muted">Your organization role does not permit workspace administration.</p>}
        </section>
      ) : null}

      {isOrgAdmin && selectedWorkspace ? (
        <section className="panel">
          <div className="workspace-section-heading"><div><h2>Member access</h2><p className="muted">Grant workspace access only to active members of this organization.</p></div></div>
          <form className="workspace-form workspace-grant-form" onSubmit={addGrant}>
            <label className="journal-field"><span>Organization member</span><select value={newUserId} onChange={(event) => setNewUserId(event.target.value)} required><option value="">Choose active member</option>{grantableMembers.map((member) => <option key={member.userId} value={member.userId}>{member.userId} · {member.role.replaceAll("_", " ")}</option>)}</select></label>
            <label className="journal-field"><span>Workspace role</span><select value={newRole} onChange={(event) => setNewRole(event.target.value as WorkspaceGrant["role"])}>{roles.map((role) => <option key={role} value={role}>{role.replaceAll("_", " ")}</option>)}</select></label>
            <button className="btn primary" type="submit" disabled={busy || !newUserId.trim() || grantableMembers.length === 0}>Grant access</button>
          </form>
          {grantsLoading ? <div className="loading">Loading access grants…</div> : grants.length === 0 ? <div className="empty">No explicit access grants are recorded for this workspace.</div> : (
            <div className="data-table-wrap"><table className="data-table workspace-grants-table"><thead><tr><th>User ID</th><th>Role</th><th>Status</th><th>Actions</th></tr></thead><tbody>
              {grants.map((grant) => <tr key={grant.userId}>
                <td><code>{grant.userId}</code></td>
                <td><select aria-label={`Role for ${grant.userId}`} value={grant.role} disabled={busy || grant.status !== "ACTIVE"} onChange={(event) => void updateGrant(grant, { role: event.target.value as WorkspaceGrant["role"] })}>{roles.map((role) => <option key={role} value={role}>{role.replaceAll("_", " ")}</option>)}</select></td>
                <td><span className={`workspace-grant-status ${grant.status.toLowerCase()}`}>{grant.status}</span></td>
                <td>{grant.status === "ACTIVE" ? <button type="button" className="btn danger" disabled={busy} onClick={() => void updateGrant(grant, { status: "REVOKED" })}>Revoke</button> : <button type="button" className="btn" disabled={busy} onClick={() => void updateGrant(grant, { status: "ACTIVE" })}>Reactivate</button>}</td>
              </tr>)}
            </tbody></table></div>
          )}
        </section>
      ) : null}
    </div>
  );
}
