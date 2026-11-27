"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useOrganization } from "@/features/organization/OrganizationProvider";
import { apiRequest } from "@/lib/api/client";

type OrganizationRole = "OWNER" | "ADMIN" | "TRADER" | "RISK_MANAGER" | "VIEWER";
type MembershipStatus = "ACTIVE" | "SUSPENDED" | "REVOKED";
type OrganizationMember = {
  organizationId: string;
  userId: string;
  role: OrganizationRole;
  status: MembershipStatus;
  revision: number;
  createdAt?: string;
  updatedAt?: string;
};
type Envelope<T> = { data: T };

const assignableRoles: Exclude<OrganizationRole, "OWNER">[] = ["ADMIN", "TRADER", "RISK_MANAGER", "VIEWER"];
const membershipStatuses: MembershipStatus[] = ["ACTIVE", "SUSPENDED", "REVOKED"];

function messageOf(error: unknown) {
  return error instanceof Error ? error.message : "The request could not be completed. Please try again.";
}

export default function OrganizationSettingsPage() {
  const {
    organizations, hasMemberships, activeOrganization, activeOrganizationId,
    loading: organizationsLoading, error: organizationError,
    refreshOrganizations, createOrganization,
  } = useOrganization();

  const [organizationName, setOrganizationName] = useState("");
  const [members, setMembers] = useState<OrganizationMember[]>([]);
  const [membersLoading, setMembersLoading] = useState(false);
  const [membersError, setMembersError] = useState<string | null>(null);
  const [newMemberId, setNewMemberId] = useState("");
  const [newMemberRole, setNewMemberRole] = useState<Exclude<OrganizationRole, "OWNER">>("VIEWER");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const loadMembers = useCallback(async () => {
    if (!activeOrganizationId || !activeOrganization || !["OWNER", "ADMIN"].includes(activeOrganization.membershipRole)) {
      setMembers([]);
      setMembersError(null);
      return;
    }
    setMembersLoading(true);
    setMembersError(null);
    try {
      const response = await apiRequest<Envelope<OrganizationMember[]>>(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/members`,
        { headers: { "X-Organization-Id": activeOrganizationId } },
      );
      setMembers(response.data ?? []);
    } catch (error) {
      setMembers([]);
      setMembersError(messageOf(error));
    } finally {
      setMembersLoading(false);
    }
  }, [activeOrganizationId, activeOrganization]);

  useEffect(() => { void loadMembers(); }, [loadMembers]);

  async function submitOrganization(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = organizationName.trim();
    if (!name) return;
    setBusy(true); setFailure(null); setNotice(null);
    try {
      await createOrganization(name);
      setOrganizationName("");
      await refreshOrganizations();
      setNotice("Organization and its default workspace were created.");
    } catch (error) {
      setFailure(messageOf(error));
    } finally {
      setBusy(false);
    }
  }

  async function addMember(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeOrganizationId) return;
    const userId = newMemberId.trim();
    const auditReason = reason.trim();
    if (!userId || !auditReason) {
      setFailure("Enter an account user ID and an audit reason.");
      return;
    }
    setBusy(true); setFailure(null); setNotice(null);
    try {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/members`,
        {
          method: "POST",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({ user_id: userId, role: newMemberRole, reason: auditReason }),
        },
      );
      setNewMemberId("");
      await loadMembers();
      await refreshOrganizations();
      setNotice("Organization membership added.");
    } catch (error) {
      setFailure(messageOf(error));
    } finally {
      setBusy(false);
    }
  }

  async function reviseMember(member: OrganizationMember, patch: { role?: OrganizationRole; status?: MembershipStatus }) {
    if (!activeOrganizationId) return;
    const auditReason = reason.trim();
    if (!auditReason) {
      setFailure("Enter an audit reason before changing organization membership.");
      return;
    }
    setBusy(true); setFailure(null); setNotice(null);
    try {
      await apiRequest(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/members/${encodeURIComponent(member.userId)}`,
        {
          method: "PATCH",
          headers: { "X-Organization-Id": activeOrganizationId },
          body: JSON.stringify({
            expected_revision: member.revision,
            role: patch.role ?? member.role,
            status: patch.status ?? member.status,
            reason: auditReason,
          }),
        },
      );
      await loadMembers();
      await refreshOrganizations();
      setNotice("Organization membership updated.");
    } catch (error) {
      setFailure(messageOf(error));
      await loadMembers();
    } finally {
      setBusy(false);
    }
  }

  const canAdminister = activeOrganization?.membershipRole === "OWNER" || activeOrganization?.membershipRole === "ADMIN";

  return (
    <div className="workspace-settings">
      <header className="page-header">
        <div>
          <span className="eyebrow">ORGANIZATION CONTROL PLANE</span>
          <h1>Organization settings</h1>
          <p>Manage your organization, membership and access boundary.</p>
        </div>
        <button className="btn" type="button" onClick={() => { void refreshOrganizations(); void loadMembers(); }} disabled={organizationsLoading || membersLoading || busy}>Refresh</button>
      </header>

      {notice ? <div className="workspace-notice" role="status">{notice}</div> : null}
      {failure || organizationError ? <div className="error" role="alert">{failure ?? organizationError}</div> : null}

      <section className="panel">
        <div className="workspace-section-heading">
          <div><h2>Your organizations</h2><p className="muted">Organization membership controls access to operational workspaces.</p></div>
          <span className="workspace-count">{organizations.length} active</span>
        </div>
        {organizationsLoading ? <div className="loading" role="status">Loading organizations…</div> : organizations.length ? (
          <div className="workspace-list">
            {organizations.map((organization) => (
              <div key={organization.id} className={`workspace-choice${organization.id === activeOrganizationId ? " selected" : ""}`}>
                <span className="workspace-choice-main"><strong>{organization.name}</strong><small>{organization.membershipRole.replaceAll("_", " ")}</small></span>
                <span className="workspace-choice-meta">{organization.id === activeOrganizationId ? <span className="workspace-tag current">ACTIVE</span> : null}</span>
              </div>
            ))}
          </div>
        ) : !hasMemberships ? <div className="empty">You do not belong to an active organization yet. Create one to provision your organization and its default workspace.</div> : <div className="empty">No active organizations are available to your account.</div>}
      </section>

      {!hasMemberships ? (
        <section className="panel">
          <h2>Create organization</h2>
          <p className="muted">Creation is atomic: the API provisions the organization, owner membership and default workspace together.</p>
          <form className="workspace-form" onSubmit={submitOrganization}>
            <label className="journal-field"><span>Organization name</span><input value={organizationName} onChange={(event) => setOrganizationName(event.target.value)} maxLength={120} required placeholder="e.g. EdgeNest Research" /></label>
            <button className="btn primary" type="submit" disabled={busy || organizationsLoading || !organizationName.trim()}>Create organization</button>
          </form>
        </section>
      ) : null}

      {activeOrganizationId && activeOrganization ? (
        <section className="panel">
          <div className="workspace-section-heading">
            <div><h2>Organization members</h2><p className="muted">Manage organization-level membership. Workspace access grants are managed separately in Workspace settings.</p></div>
            <span className="workspace-tag">{activeOrganization.membershipRole.replaceAll("_", " ")}</span>
          </div>
          {!canAdminister ? <p className="muted">Only organization owners and administrators can manage membership.</p> : (
            <>
              <p className="muted">Add an existing EdgeNest account by its user ID. This API does not currently issue email invitations; do not enter an email address unless it is the account's actual user ID.</p>
              <form className="workspace-form workspace-grant-form" onSubmit={addMember}>
                <label className="journal-field"><span>Existing account user ID</span><input value={newMemberId} onChange={(event) => setNewMemberId(event.target.value)} required placeholder="User ID" /></label>
                <label className="journal-field"><span>Organization role</span><select value={newMemberRole} onChange={(event) => setNewMemberRole(event.target.value as Exclude<OrganizationRole, "OWNER">)}>{assignableRoles.map((role) => <option key={role} value={role}>{role.replaceAll("_", " ")}</option>)}</select></label>
                <label className="journal-field"><span>Audit reason</span><input value={reason} onChange={(event) => setReason(event.target.value)} maxLength={500} required placeholder="Why is this membership being added?" /></label>
                <button className="btn primary" type="submit" disabled={busy || !newMemberId.trim() || !reason.trim()}>Add member</button>
              </form>
              {membersError ? <div className="error" role="alert">{membersError}</div> : null}
              {membersLoading ? <div className="loading" role="status">Loading organization members…</div> : members.length === 0 ? <div className="empty">No organization members were returned.</div> : (
                <div className="data-table-wrap">
                  <table className="data-table workspace-grants-table">
                    <thead><tr><th>User ID</th><th>Role</th><th>Status</th><th>Membership revision</th><th>Update</th></tr></thead>
                    <tbody>{members.map((member) => (
                      <tr key={member.userId}>
                        <td><code>{member.userId}</code></td>
                        <td>
                          {member.role === "OWNER" ? <span className="workspace-tag">OWNER</span> : (
                            <select aria-label={`Role for ${member.userId}`} value={member.role} disabled={busy || member.status !== "ACTIVE"} onChange={(event) => void reviseMember(member, { role: event.target.value as OrganizationRole })}>
                              {assignableRoles.map((role) => <option key={role} value={role}>{role.replaceAll("_", " ")}</option>)}
                            </select>
                          )}
                        </td>
                        <td><span className={`workspace-grant-status ${member.status.toLowerCase()}`}>{member.status}</span></td>
                        <td>{member.revision}</td>
                        <td>{member.role === "OWNER" ? <span className="muted">Owner transfer only</span> : (
                          <select aria-label={`Status for ${member.userId}`} value={member.status} disabled={busy} onChange={(event) => void reviseMember(member, { status: event.target.value as MembershipStatus })}>
                            {membershipStatuses.map((status) => <option key={status} value={status}>{status}</option>)}
                          </select>
                        )}</td>
                      </tr>
                    ))}</tbody>
                  </table>
                </div>
              )}
            </>
          )}
        </section>
      ) : null}
    </div>
  );
}
