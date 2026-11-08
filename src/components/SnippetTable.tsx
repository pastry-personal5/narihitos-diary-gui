import { useEffect, useState } from "react";
import ayumiIcon from "../assets/source-ayumi.png";
import narihitoIcon from "../assets/source-narihito.png";
import trashIcon from "../assets/trash.png";
import type {
  Source,
  SourceGroup,
  SourceItem,
  TimeAuthority,
} from "../application/navigation/contracts";
import type { NavigationModel } from "../application/navigation/model";
import { ModalDialog } from "./ModalDialog";
import detailsIcon from "../assets/state-details.png";
import readyIcon from "../assets/state-ready.png";
import problemIcon from "../assets/state-problem.png";
import downloadedIcon from "../assets/state-downloaded.png";
import notDownloadedIcon from "../assets/state-not-downloaded.png";

const typeIcons = { ayumi: ayumiIcon, narihito: narihitoIcon };
const typeNames = { ayumi: "Ayumi", narihito: "Narihito" };

type Context = NonNullable<ReturnType<NavigationModel["imports"]["view"]>>;
type Choice = { source: Source; authority: TimeAuthority } | null;

/** A custom bitmap, tinted like the other app icons. */
function Bitmap({ src, size = 18 }: { src: string; size?: number }) {
  return (
    <span
      aria-hidden="true"
      className="bitmap-icon"
      style={{
        width: size,
        height: size,
        maskImage: `url(${src})`,
        WebkitMaskImage: `url(${src})`,
      }}
    />
  );
}

export function SnippetTable({
  model,
  context,
  date,
  labels,
  groups,
}: {
  model: NavigationModel;
  context: Context;
  date: string | null;
  labels: Map<string, string>;
  groups: SourceGroup[];
}) {
  const [detail, setDetail] = useState<string | null>(null);
  const [choice, setChoice] = useState<Choice>(null);
  const keyOf = (item: SourceItem, index: number) =>
    "source" in item && item.source ? item.source.id : `row-${index}`;
  const groupOf = (item: SourceItem) =>
    item.kind === "included" || item.kind === "warning"
      ? item.source.navigation?.group_id
      : item.group_id;
  const close = () => {
    setDetail(null);
    setChoice(null);
  };

  /** Everything the table row and the Detail overlay say about one item. */
  function describe(item: SourceItem) {
    const source = "source" in item ? item.source : null;
    const group = groups.find((g) => g.id === groupOf(item));
    if (!source) {
      const code = item.kind === "failure" ? item.code : "path_collision";
      const name =
        item.kind === "path_collision"
          ? `Path collision: ${item.relative_keys.join(", ")}`
          : item.kind === "failure"
            ? item.relative_key || "Source root"
            : "Source root";
      return {
        source,
        group,
        name,
        label: name,
        status: { icon: problemIcon, text: "Problem" },
        local:
          code === "not_downloaded"
            ? { icon: notDownloadedIcon, text: "Not downloaded" }
            : null,
        notes: [`Problem: ${code.replaceAll("_", " ")}.`],
        warning: false,
        moved: false,
      };
    }
    const label = labels.get(source.id) ?? source.relative_key;
    const local = source.navigation?.included;
    const inclusion = model.imports.inclusion(source.id);
    const moved = model.imports.sourceDisabled(source);
    const failed = item.kind === "failure";
    const warning =
      item.kind !== "included" ||
      source.dating.state !== "agreed" ||
      source.duplicate_producer_id;
    const notes: string[] = [];
    if (moved)
      notes.push(
        "Moved to Trash. Refresh snippets before using this row again.",
      );
    if (inclusion?.invalidated)
      notes.push(`Selection invalidated: ${inclusion.invalidated}.`);
    if (failed) notes.push(`Problem: ${item.code.replaceAll("_", " ")}.`);
    return {
      source,
      group,
      label,
      name: local && labels.has(source.id) ? `${local.date} ${label}` : label,
      status: moved
        ? { icon: trashIcon, text: "Moved to Trash" }
        : failed
          ? { icon: problemIcon, text: "Problem" }
          : warning || inclusion?.invalidated
            ? { icon: problemIcon, text: "Needs attention" }
            : { icon: readyIcon, text: "Ready" },
      // A failure other than "not downloaded" says nothing about the local copy.
      local:
        failed && item.code === "not_downloaded"
          ? { icon: notDownloadedIcon, text: "Not downloaded" }
          : failed
            ? null
            : { icon: downloadedIcon, text: "Downloaded" },
      notes,
      warning,
      moved,
    };
  }

  const items = context.items;
  const open =
    detail === null ? null : items.find((it, i) => keyOf(it, i) === detail);
  const info = open ? describe(open) : null;

  return (
    <>
      <table className="snippet-table" aria-label="Snippets">
        <thead>
          <tr>
            <th scope="col">
              <span className="visually-hidden">Include</span>
            </th>
            <th scope="col">
              <span className="visually-hidden">Source</span>
            </th>
            <th scope="col">Name</th>
            <th scope="col">
              <span className="visually-hidden">State</span>
            </th>
            <th scope="col">
              <span className="visually-hidden">Detail</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {items.map((item, index) => {
            const d = describe(item);
            const { source } = d;
            const inclusion = source && model.imports.inclusion(source.id);
            const trashReason = source && model.trashReason(source);
            return (
              <tr
                key={keyOf(item, index)}
                onFocusCapture={() => source && model.focusSnippet(source)}
              >
                <td>
                  {source && (
                    <input
                      type="checkbox"
                      aria-label={`Include ${d.label}`}
                      checked={
                        !!inclusion?.checked && source.eligible && !d.moved
                      }
                      disabled={!source.eligible || !!context.stale || d.moved}
                      onChange={(e) =>
                        date &&
                        model.imports.setIncluded(
                          date,
                          source.id,
                          e.target.checked,
                        )
                      }
                    />
                  )}
                </td>
                <td
                  title={
                    d.group
                      ? `${typeNames[d.group.type]} · ${d.group.alias}`
                      : "Unknown source"
                  }
                >
                  {d.group ? (
                    <>
                      <Bitmap src={typeIcons[d.group.type]} />
                      <span className="visually-hidden">
                        {typeNames[d.group.type]}
                      </span>
                    </>
                  ) : (
                    "—"
                  )}
                </td>
                <td>
                  {source ? (
                    <button
                      disabled={
                        !source.capabilities.read || !!context.stale || d.moved
                      }
                      title={source.relative_key}
                      onClick={() => void model.openSource(source)}
                    >
                      {d.name}
                    </button>
                  ) : (
                    d.name
                  )}
                </td>
                <td className="snippet-state">
                  <button
                    className="snippet-trash"
                    title={d.status.text}
                    aria-label={`${d.status.text}: ${d.label}`}
                    onClick={() => setDetail(keyOf(item, index))}
                  >
                    <Bitmap src={d.status.icon} size={14} />
                  </button>
                  {d.local && (
                    <button
                      className="snippet-trash"
                      title={d.local.text}
                      aria-label={`${d.local.text}: ${d.label}`}
                      onClick={() => setDetail(keyOf(item, index))}
                    >
                      <Bitmap src={d.local.icon} size={14} />
                    </button>
                  )}
                </td>
                <td className="snippet-actions">
                  <button
                    className="snippet-trash"
                    title="Details"
                    aria-label={`Details of ${d.label}`}
                    onClick={() => setDetail(keyOf(item, index))}
                  >
                    <Bitmap src={detailsIcon} size={14} />
                  </button>
                  {source && (
                    <button
                      className="snippet-trash"
                      disabled={!!trashReason}
                      title={
                        trashReason ??
                        "Preview this snippet and its related audio"
                      }
                      aria-label={`Move ${d.label} to Trash`}
                      onClick={() => void model.trashSource(source)}
                    >
                      <Bitmap src={trashIcon} size={16} />
                    </button>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {info && (
        <ModalDialog label="Snippet details" onDismiss={close}>
          <h3>{info.name}</h3>
          <p>
            Source:{" "}
            {info.group
              ? `${typeNames[info.group.type]} (${info.group.alias})`
              : "unknown"}
            .
          </p>
          {info.source && <p>File: {info.source.relative_key}</p>}
          <p>State: {info.status.text}.</p>
          <p>Local copy: {info.local ? info.local.text : "unknown"}.</p>
          {info.notes.map((note) => (
            <p key={note}>{note}</p>
          ))}
          {info.source && info.warning && (
            <MetadataDetail
              source={info.source}
              moved={info.moved}
              context={context}
              choice={choice}
              setChoice={setChoice}
              onResolve={(authority) =>
                date &&
                info.source &&
                void model.imports.resolve(date, info.source, authority)
              }
            />
          )}
          {info.source && (
            <SourceText
              key={info.source.id}
              model={model}
              source={info.source}
            />
          )}
          <button data-initial-focus onClick={close}>
            Close
          </button>
        </ModalDialog>
      )}
    </>
  );
}

function MetadataDetail({
  source,
  moved,
  context,
  choice,
  setChoice,
  onResolve,
}: {
  source: Source;
  moved: boolean;
  context: Context;
  choice: Choice;
  setChoice: (c: Choice) => void;
  onResolve: (authority: TimeAuthority) => void;
}) {
  return (
    <>
      <h4>Metadata warning</h4>
      <p>
        Timestamp status: {source.dating.state.replaceAll("_", " ")}
        {source.duplicate_producer_id ? "; duplicate producer ID" : ""}.
      </p>
      <p>
        Header:{" "}
        {source.navigation?.header?.datetime ??
          source.header_invalid ??
          "unavailable"}
      </p>
      <p>
        Filename:{" "}
        {source.navigation?.filename?.datetime ??
          source.filename_invalid ??
          "unavailable"}
      </p>
      {source.navigation?.authority && (
        <p>Chosen authority: {source.navigation.authority}.</p>
      )}
      {(["header", "filename"] as const).map((authority) => (
        <button
          key={authority}
          disabled={
            !source.navigation?.[authority] ||
            !!context.stale ||
            moved ||
            !!context.resolving
          }
          onClick={() => setChoice({ source, authority })}
        >
          Use {authority} time
        </button>
      ))}
      {!moved && choice?.source.id === source.id && (
        <p>
          Use {choice.authority} time for a new import?{" "}
          <button
            onClick={() => {
              onResolve(choice.authority);
              setChoice(null);
            }}
          >
            Confirm authority
          </button>{" "}
          <button onClick={() => setChoice(null)}>Cancel</button>
        </p>
      )}
    </>
  );
}

/** Raw file text (metadata header, then body), loaded on demand and shown read-only. */
function SourceText({
  model,
  source,
}: {
  model: NavigationModel;
  source: Source;
}) {
  const [text, setText] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let live = true;
    model
      .readSource(source)
      .then((r) => {
        if (!live) return;
        setText(
          r.front_matter
            ? `${r.front_matter}${r.front_matter.endsWith("\n") ? "" : "\n"}\n${r.body}`
            : r.body,
        );
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [model, source]);
  if (failed) return <p role="alert">The file text could not be read.</p>;
  if (text === null) return <p role="status">Loading file text…</p>;
  return (
    <pre className="snippet-text" aria-label="File text" tabIndex={0}>
      {text}
    </pre>
  );
}
