import type { ProfileCapability, ProfileInfo } from "../application/bridge";
import { SymbolLabel } from "./Symbol";
import {
  PROFILE_REASON,
  PROFILE_STATUS,
  PROFILE_STATUS_REASON_TEXT,
  type useProfiles,
} from "./useProfiles";

// [SF Symbol, tone class] per status.
const STATUS_ICON: Record<ProfileInfo["status"], [string, string]> = {
  unchecked: ["circle.dotted", "muted"],
  missing: ["xmark.circle.fill", "danger"],
  unsupported_version: ["exclamationmark.triangle.fill", "warning"],
  unavailable: ["xmark.circle.fill", "danger"],
  available_auth_unknown: ["questionmark.circle", "warning"],
  unauthenticated: ["lock", "warning"],
  ready: ["checkmark.circle.fill", "success"],
  busy: ["hourglass", "warning"],
  failed: ["exclamationmark.triangle.fill", "danger"],
};

function Capability({ cap }: { cap: ProfileCapability }) {
  return cap.available ? (
    <span className="tone-success">
      <SymbolLabel name="checkmark" label="Yes" />
    </span>
  ) : (
    <span className="tone-muted">
      <SymbolLabel name="minus" label="No" />
      {cap.reason && <small> {PROFILE_REASON[cap.reason] ?? cap.reason}</small>}
    </span>
  );
}

export function ProfileTable({
  profiles,
}: {
  profiles: ReturnType<typeof useProfiles>;
}) {
  const list = profiles.profiles;
  return (
    <>
      <table className="profile-table" aria-label="Profile status">
        <thead>
          <tr>
            <th>Profile</th>
            <th>Status</th>
            <th>Chat</th>
            <th>Generate</th>
            <th>
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {list?.length === 0 && (
            <tr>
              <td colSpan={5}>None configured</td>
            </tr>
          )}
          {list?.map((p) => {
            const busy = profiles.checking.includes(p.alias);
            const [icon, tone] = busy
              ? ["arrow.triangle.2.circlepath", "muted"]
              : STATUS_ICON[p.status];
            return (
              <tr key={p.alias}>
                <td>
                  <span className="tone-muted">
                    <SymbolLabel
                      name={
                        p.type === "apple-foundation-models"
                          ? "cpu"
                          : "terminal"
                      }
                      label=""
                      fallback=""
                    />
                  </span>{" "}
                  <strong>{p.alias}</strong> <small>{p.type}</small>
                </td>
                <td>
                  <span className={`tone-${tone}`}>
                    <SymbolLabel name={icon} label="" fallback="" />
                  </span>{" "}
                  {PROFILE_STATUS[p.status]}
                  {busy && <small> · checking…</small>}
                  {p.reason && (
                    <small>
                      {" "}
                      · {PROFILE_STATUS_REASON_TEXT[p.reason] ?? ""}
                    </small>
                  )}
                </td>
                <td>
                  <Capability cap={p.capabilities.chat} />
                </td>
                <td>
                  <Capability cap={p.capabilities.generate} />
                </td>
                <td>
                  <button
                    disabled={busy}
                    aria-label={`Check ${p.alias}`}
                    onClick={() => void profiles.check(p.alias)}
                  >
                    <SymbolLabel
                      name="arrow.clockwise"
                      label="Check"
                      fallback="Check"
                    />
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p>
        <small>
          Checked in the background after launch, then again after 15 s, 30 s, 1
          min and so on, up to once an hour.
        </small>
      </p>
      {profiles.error && <p role="alert">{profiles.error}</p>}
    </>
  );
}
