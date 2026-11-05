// Producer round-trip harness: runs the pinned mobile codec on a file. Built by producer.py.
import Foundation

func fail(_ text: String) -> Never {
  FileHandle.standardError.write(Data((text + "\n").utf8))
  exit(2)
}
let args = CommandLine.arguments
guard args.count >= 3 else { fail("usage: producer decode|edit|toggle-favorite FILE ...") }
let url = URL(fileURLWithPath: args[2])
let data = try Data(contentsOf: url)
let codec = DiarySnippetMarkdownCodec()

func supported() -> DiarySnippetV1 {
  switch codec.decode(data) {
  case .supported(let snippet): return snippet
  case .unsupported(let reason, _): print("unsupported:\(reason)"); exit(1)
  case .malformed(let code): print("malformed:\(code)"); exit(1)
  }
}
// The `updated_at` rule of `DiaryEditRevision.make`: now if later, else previous + 1 s.
func stamp(_ previous: SchemaInstant, now: String) throws -> SchemaInstant {
  let current = try SchemaInstant(now)
  if current.description > previous.description { return current }
  guard let date = ISO8601DateFormatter().date(from: previous.description) else { fail("bad previous") }
  return try SchemaInstant.from(date: date.addingTimeInterval(1))
}

switch args[1] {
case "decode":
  let s = supported()
  print("supported")
  print("updated_at=\(s.updatedAt.description)")
  print("created_at=\(s.createdAt.description)")
  print("title=\(s.title)")
  print("title_origin=\(s.titleProvenance.originID.description)")
  print("favorite=\(s.favorite)")
  print("language=\(s.language)")
  print("duration=\(s.recordingDurationMilliseconds)")
  print("preserved=\(s.preservedScalars.map { "\($0.key)=\($0.rawValue)" }.joined(separator: "|"))")
  print("body_b64=\(Data(s.body.text.utf8).base64EncodedString())")
case "edit":
  guard args.count == 5 else { fail("edit FILE BODY_FILE NOW") }
  var s = supported()
  s.body = try DiaryBody(String(contentsOf: URL(fileURLWithPath: args[3]), encoding: .utf8))
  s.updatedAt = try stamp(s.updatedAt, now: args[4])
  try codec.encode(s).write(to: url)
case "toggle-favorite":
  guard args.count == 4 else { fail("toggle-favorite FILE NOW") }
  var s = supported()
  s.favorite.toggle()
  s.updatedAt = try stamp(s.updatedAt, now: args[3])
  try codec.encode(s).write(to: url)
default:
  fail("unknown command")
}
