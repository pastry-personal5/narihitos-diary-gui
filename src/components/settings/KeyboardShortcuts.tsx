import { useCallback, useState } from "react";
import {
  ACTIONS,
  DEFAULT_BINDINGS,
  conflictFor,
  type ActionInfo,
} from "../../application/actions/actions";
import {
  composing,
  fromEvent,
  spoken,
  validate,
} from "../../application/actions/shortcut";
import type { UserSettings } from "../../application/bridge";

const ISSUE: Record<string, string> = {
  unknown_action: "is not a known action and is inactive.",
  invalid_shortcut: "has an invalid shortcut that is inactive.",
  reserved: "uses a reserved key that is inactive.",
  conflict:
    "has a shortcut that conflicts with another action and is inactive.",
};
const same = (a: string[], b: string[]) =>
  a.length === b.length && a.every((s, i) => s === b[i]);
const Keys = ({ list }: { list: string[] }) =>
  list.length === 0 ? (
    <>None</>
  ) : (
    <>
      {list.map((s) => (
        <kbd key={s} aria-label={spoken(s)}>
          {s}{" "}
        </kbd>
      ))}
    </>
  );

export function KeyboardShortcuts({
  user,
  save,
}: {
  user: UserSettings | undefined;
  save: (value: unknown) => Promise<boolean>;
}) {
  const bindings = user?.effective_keybindings ?? DEFAULT_BINDINGS;
  const readOnly = !user || user.read_only || user.recoverable;
  const [editing, setEditing] = useState<string>();
  const [capturing, setCapturing] = useState(false);
  const [message, setMessage] = useState("");
  const [reassign, setReassign] = useState<{
    shortcut: string;
    owner: ActionInfo;
  }>();
  const [confirmAll, setConfirmAll] = useState(false);
  // React only honours autoFocus on form controls, so focus the capture box by ref.
  const focusCapture = useCallback(
    (el: HTMLDivElement | null) => el?.focus(),
    [],
  );
  const stop = () => {
    setCapturing(false);
    setReassign(undefined);
  };
  const add = (action: ActionInfo, shortcut: string) => {
    const owner = conflictFor(action.id, shortcut, bindings);
    if (owner) {
      setMessage(`${shortcut} is used by ${owner.label}.`);
      setReassign({ shortcut, owner });
      return;
    }
    stop();
    setMessage("");
    void save({
      set: { [action.id]: [...bindings[action.id], shortcut] },
    });
  };
  const capture = (e: React.KeyboardEvent, action: ActionInfo) => {
    if (composing(e.nativeEvent)) return;
    e.preventDefault();
    if (e.key === "Escape") {
      stop();
      setMessage("");
      return;
    }
    const shortcut = fromEvent(e.nativeEvent);
    if (!shortcut) return;
    const bad = validate(action.scope, shortcut);
    if (bad) {
      setMessage(
        bad === "reserved"
          ? `${shortcut} is reserved for text editing or the system.`
          : `${shortcut} is not a valid shortcut.`,
      );
      return;
    }
    add(action, shortcut);
  };
  return (
    <div className="card" aria-label="Keyboard Shortcuts">
      <h2>Keyboard Shortcuts</h2>
      {user?.keybinding_issues.length ? (
        <ul aria-label="Shortcut issues">
          {user.keybinding_issues.map((i, n) => (
            <li key={`${n}.${i.id}.${i.code}`}>
              {ACTIONS.find((a) => a.id === i.id)?.label ?? i.id}{" "}
              {ISSUE[i.code] ?? "has a problem."}
            </li>
          ))}
        </ul>
      ) : null}
      <button disabled={readOnly} onClick={() => setConfirmAll(true)}>
        Reset all
      </button>
      {confirmAll && (
        <div role="dialog" aria-label="Reset all shortcuts">
          <p>Restore every shortcut to its original value?</p>
          <button
            onClick={() => {
              setConfirmAll(false);
              void save({ reset_all: true });
            }}
          >
            Confirm reset all
          </button>
          <button onClick={() => setConfirmAll(false)}>Cancel</button>
        </div>
      )}
      <table>
        <thead>
          <tr>
            <th>Action</th>
            <th>Original shortcuts</th>
            <th>Current shortcuts</th>
            <th>Info</th>
            <th>Controls</th>
          </tr>
        </thead>
        <tbody>
          {ACTIONS.map((a) => {
            const current = bindings[a.id] ?? [];
            const open = editing === a.id;
            return (
              <tr key={a.id}>
                <td>{a.label}</td>
                <td>
                  <Keys list={a.defaults} />
                </td>
                <td>
                  <Keys list={current} />
                  {!same(current, a.defaults) && <em> Modified</em>}
                  {open && (
                    <ul aria-label={`${a.label} shortcuts`}>
                      {current.map((s) => (
                        <li key={s}>
                          {s}{" "}
                          <button
                            aria-label={`Remove ${spoken(s)} from ${a.label}`}
                            onClick={() =>
                              void save({
                                set: {
                                  [a.id]: current.filter((x) => x !== s),
                                },
                              })
                            }
                          >
                            Remove
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </td>
                <td>
                  {a.scope === "app" ? "Anywhere" : "Snippet tree"}: {a.info}
                  {open && message && <p role="alert">{message}</p>}
                  {open && reassign && (
                    <button
                      onClick={() => {
                        const { shortcut, owner } = reassign;
                        stop();
                        setMessage("");
                        void save({
                          set: {
                            [a.id]: [...current, shortcut],
                            [owner.id]: bindings[owner.id].filter(
                              (x) => x !== shortcut,
                            ),
                          },
                        });
                      }}
                    >
                      Reassign
                    </button>
                  )}
                </td>
                <td>
                  <button
                    disabled={readOnly}
                    aria-label={`Edit ${a.label}`}
                    onClick={() => {
                      stop();
                      setMessage("");
                      setEditing(open ? undefined : a.id);
                    }}
                  >
                    {open ? "Done" : "Edit"}
                  </button>{" "}
                  <button
                    disabled={readOnly}
                    aria-label={`Reset ${a.label}`}
                    onClick={() => void save({ set: { [a.id]: null } })}
                  >
                    Reset
                  </button>
                  {open && !capturing && (
                    <button
                      aria-label={`Add shortcut to ${a.label}`}
                      onClick={() => setCapturing(true)}
                    >
                      Add shortcut
                    </button>
                  )}
                  {open && capturing && (
                    <div
                      data-capture
                      tabIndex={0}
                      ref={focusCapture}
                      role="textbox"
                      aria-label="Press a shortcut"
                      onKeyDown={(e) => capture(e, a)}
                    >
                      Press a shortcut (Escape cancels)
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
