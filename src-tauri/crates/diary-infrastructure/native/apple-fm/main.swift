// apple-fm: the only native boundary to Apple's on-device system language model.
// Commands (argv holds nothing else):
//   availability  -> one JSON line, no session, no inference
//   run           -> reads one JSON request on stdin, writes JSON lines: delta*, then result|error
// Stdin is the only input. No tools, no files, no network, no cloud model. Error output carries
// closed codes only, never native message text.
import Foundation
import FoundationModels

@Generable struct Claim {
    var claim: String
    var snippet_ids: [String]
}
// Mirrors GENERATION_SCHEMA (schema 1); a Rust test compares the field names.
@Generable struct Candidate {
    var draft_markdown: String
    var claim_support: [Claim]
    var warnings: [String]
}

func emit(_ obj: [String: Any]) {
    guard let d = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]),
        var s = String(data: d, encoding: .utf8)
    else { return }
    s += "\n"
    FileHandle.standardOutput.write(Data(s.utf8))
}

func fail(_ code: String) -> Never {
    emit(["type": "error", "code": code])
    exit(0)  // the terminal line decides; exit codes are advisory
}

func availability() {
    let model = SystemLanguageModel.default
    var out: [String: Any] = [
        "os": ProcessInfo.processInfo.operatingSystemVersionString
    ]
    switch model.availability {
    case .available:
        out["state"] = "available"
        out["context"] = model.contextSize
        out["languages"] = Array(Set(model.supportedLanguages.compactMap { $0.languageCode?.identifier })).sorted()
    case .unavailable(let reason):
        out["state"] = "unavailable"
        switch reason {
        case .deviceNotEligible: out["reason"] = "device_ineligible"
        case .appleIntelligenceNotEnabled: out["reason"] = "apple_intelligence_disabled"
        case .modelNotReady: out["reason"] = "model_not_ready"
        @unknown default: out["reason"] = "model_unavailable_unknown"
        }
    }
    emit(out)
}

func mapError(_ error: Error) -> String {
    if error is CancellationError { return "cancelled" }
    if let e = error as? LanguageModelSession.GenerationError {
        switch e {
        case .exceededContextWindowSize: return "context"
        case .assetsUnavailable: return "unavailable"
        case .guardrailViolation: return "refusal"
        case .unsupportedLanguageOrLocale: return "unsupported_language"
        case .decodingFailure: return "invalid_output"
        case .rateLimited: return "rate_limited"
        case .concurrentRequests: return "busy"
        default: return "native_error_unmapped"
        }
    }
    return "native_error_unmapped"
}

struct Request {
    var instructions: String
    var prompt: String
    var locale: String
    var generate: Bool
}

func readRequest() -> Request {
    let data = FileHandle.standardInput.readDataToEndOfFile()
    guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
        let instructions = obj["instructions"] as? String,
        let prompt = obj["prompt"] as? String,
        let locale = obj["locale"] as? String,
        let mode = obj["mode"] as? String, mode == "chat" || mode == "generate"
    else { fail("bad_request") }
    return Request(instructions: instructions, prompt: prompt, locale: locale, generate: mode == "generate")
}

func run() async {
    let req = readRequest()
    let model = SystemLanguageModel.default
    // Availability is checked again here: it can change between the check and the run.
    guard case .available = model.availability else { fail("unavailable") }
    // Free of charge: no inference has happened yet.
    guard model.supportsLocale(Locale(identifier: req.locale)) else { fail("locale_unsupported") }
    let session = LanguageModelSession(model: model, instructions: req.instructions)
    do {
        if req.generate {
            let r = try await session.respond(to: req.prompt, generating: Candidate.self)
            let c = r.content
            let obj: [String: Any] = [
                "draft_markdown": c.draft_markdown,
                "claim_support": c.claim_support.map { ["claim": $0.claim, "snippet_ids": $0.snippet_ids] },
                "warnings": c.warnings,
            ]
            guard let d = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]),
                let s = String(data: d, encoding: .utf8)
            else { fail("invalid_output") }
            emit(["type": "result", "text": s])
        } else {
            var sent = ""
            var last = ""
            for try await snapshot in session.streamResponse(to: req.prompt) {
                last = snapshot.content
                if last.hasPrefix(sent), last.count > sent.count {
                    emit(["type": "delta", "text": String(last.dropFirst(sent.count))])
                    sent = last
                }
            }
            emit(["type": "result", "text": last])
        }
    } catch {
        fail(mapError(error))
    }
}

switch CommandLine.arguments.dropFirst().first {
case "availability": availability()
case "run": await run()
default: fail("bad_request")
}
